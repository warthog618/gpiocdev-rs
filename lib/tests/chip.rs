// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

// ALL IT as can't construct a Chip without opening a GPIO file.
//
// Assumptions:
//  - kernel supports uAPI versions corresponding to selected build features

use errno::Errno;
use gpiocdev::chip::ErrorKind;
use gpiocdev::Error::GpioChip as ChipError;
use gpiosim::{Bank, Sim};
use std::path::{Path, PathBuf};

// a collection of chips for testing chip existence
fn bag_of_chips() -> Sim {
    gpiosim::builder()
        .with_bank(&Bank::new(8, "veintidós"))
        .with_bank(&Bank::new(12, "babel"))
        .with_bank(&Bank::new(6, "woggle"))
        .live()
        .unwrap()
}

struct Symlink {
    src: PathBuf,
    dst: PathBuf,
}

impl Symlink {
    fn new<P: AsRef<Path>>(dst: P, src: P) -> Result<Symlink, std::io::Error> {
        std::os::unix::fs::symlink(&dst, &src)?;
        Ok(Symlink {
            src: src.as_ref().to_path_buf(),
            dst: dst.as_ref().to_path_buf(),
        })
    }
}
impl Drop for Symlink {
    fn drop(&mut self) {
        _ = std::fs::remove_file(&self.src);
    }
}

#[test]
fn is_chip() {
    let cc = bag_of_chips();
    for c in cc.chips() {
        assert_eq!(gpiocdev::chip::is_chip(&c.dev_path).unwrap(), c.dev_path);
    }
}

#[test]
fn is_chip_symlink() {
    let cc = bag_of_chips();
    let mut path = PathBuf::from("/tmp");
    path.push(gpiosim::unique_name("gpiocdev_chip", None));
    let link = Symlink::new(&cc.chips()[0].dev_path, &path).unwrap();
    // locates and reports the path of the actual device
    assert_eq!(*gpiocdev::chip::is_chip(&link.src).unwrap(), *link.dst);
}

#[test]
fn is_chip_nonexistent() {
    let path = PathBuf::from("/dev/gpiochip_nonexistent");
    assert_eq!(
        gpiocdev::chip::is_chip(&path),
        Err(gpiocdev::Error::OsError(Errno(2)))
    );
}

#[test]
fn is_chip_not_character_device() {
    let path = PathBuf::from("/tmp");
    assert_eq!(
        gpiocdev::chip::is_chip(&path),
        Err(ChipError(path, ErrorKind::NotCharacterDevice))
    );
}

#[test]
fn is_chip_not_gpio_device() {
    let path = PathBuf::from("/dev/null");
    assert_eq!(
        gpiocdev::chip::is_chip(&path),
        Err(ChipError(path, ErrorKind::NotGpioDevice))
    );
}

#[test]
fn chips() {
    let cc = bag_of_chips();
    for c in cc.chips() {
        // all chips in the test set must be in the system
        assert!(
            gpiocdev::chip::chips().unwrap().any(|x| x == c.dev_path),
            "{:?} not found in chips",
            c.dev_path
        );
    }
}

mod chip {
    use super::*;
    use gpiocdev::chip::Chip;
    use gpiocdev::request::Request;
    use gpiocdev::Error::GpioChip as ChipError;
    use gpiosim::{Bank, Direction};
    use std::time::Duration;

    // a collection of chips with named and hogged lines for find and info tests
    fn detailed_sim() -> gpiosim::Sim {
        gpiosim::builder()
            .with_bank(
                Bank::new(8, "veintidós")
                    .name(3, "banana")
                    .name(5, "apple")
                    .hog(2, "hogster", Direction::OutputLow)
                    .hog(5, "breath", Direction::Input),
            )
            .with_bank(
                Bank::new(42, "babel")
                    .name(3, "piñata")
                    .name(4, "piggly")
                    .hog(7, "hogster", Direction::OutputHigh),
            )
            .live()
            .unwrap()
    }

    #[test]
    fn from_path() {
        let sim = bag_of_chips();
        for c in sim.chips() {
            assert!(
                Chip::from_path(&c.dev_path).is_ok(),
                "failed to open {:?}",
                c.dev_path
            );
        }
    }

    #[test]
    fn from_path_symlink() {
        let cc = bag_of_chips();
        let mut path = PathBuf::from("/tmp");
        path.push(gpiosim::unique_name("gpiocdev_chip", None));
        let link = Symlink::new(&cc.chips()[0].dev_path, &path).unwrap();
        // constructs and reports the path of the actual device
        assert_eq!(Chip::from_path(&link.src).unwrap().path(), link.dst);
    }

    #[test]
    fn from_path_nonexistent() {
        let path = PathBuf::from("/dev/gpiochip_nonexistent");
        assert_eq!(
            Chip::from_path(&path).unwrap_err(),
            gpiocdev::Error::OsError(Errno(2))
        );
    }

    #[test]
    fn from_path_not_character_device() {
        let path = PathBuf::from("/tmp");
        assert_eq!(
            Chip::from_path(&path).unwrap_err(),
            ChipError(path, ErrorKind::NotCharacterDevice)
        );
    }

    #[test]
    fn from_path_no_gpio_device() {
        let path = PathBuf::from("/dev/null");
        assert_eq!(
            Chip::from_path(&path).unwrap_err(),
            ChipError(path, ErrorKind::NotGpioDevice)
        );
    }

    #[test]
    fn info() {
        let sim = bag_of_chips();
        for simc in sim.chips() {
            let cdevc = Chip::from_path(&simc.dev_path).unwrap();
            let info = cdevc.info().unwrap();
            assert_eq!(info.num_lines, simc.cfg.num_lines);
            assert_eq!(info.label.as_str(), simc.cfg.label);
            assert_eq!(info.name.as_str(), simc.chip_name);
        }
    }

    #[test]
    fn name() {
        let sim = bag_of_chips();
        for simc in sim.chips() {
            let ch = Chip::from_path(&simc.dev_path).unwrap();
            assert_eq!(ch.name().as_str(), simc.chip_name);
        }
    }

    #[test]
    fn path() {
        let sim = bag_of_chips();
        for simc in sim.chips() {
            let cdevc = Chip::from_path(&simc.dev_path).unwrap();
            assert_eq!(cdevc.path(), simc.dev_path);
        }
    }

    #[test]
    fn find_line() {
        let sim = detailed_sim();
        for simc in sim.chips() {
            let cdevc = Chip::from_path(&simc.dev_path).unwrap();
            for (offset, name) in &simc.cfg.names {
                assert_eq!(cdevc.find_line(name), Some(*offset));
            }

            // non-existent
            assert!(cdevc.find_line("not such line").is_none())
        }
    }

    fn line_info(abiv: gpiocdev::AbiVersion) {
        let sim = detailed_sim();
        for simc in sim.chips() {
            let cdevc = new_chip(&simc.dev_path, abiv);
            // names, offsets and used
            for (offset, name) in &simc.cfg.names {
                let info = cdevc.line_info(*offset).unwrap();
                assert_eq!(info.name.as_str(), name);
                assert_eq!(info.offset, *offset);
                assert_eq!(info.used, simc.cfg.hogs.contains_key(offset))
            }

            // consumer, direction and used
            for (offset, hog) in &simc.cfg.hogs {
                let info = cdevc.line_info(*offset).unwrap();
                assert_eq!(info.consumer.as_str(), &hog.name);
                assert_eq!(info.offset, *offset);
                assert_eq!(
                    info.direction,
                    match &hog.direction {
                        Direction::Input => gpiocdev::line::Direction::Input,
                        Direction::OutputLow => gpiocdev::line::Direction::Output,
                        Direction::OutputHigh => gpiocdev::line::Direction::Output,
                    }
                );
                assert!(info.used);
            }
        }

        // config menagerie on simpleton
        let sim = gpiosim::simpleton(10);
        let simc = sim.chip();
        let offset = 1;
        // Combinations covering all possible active low, bias, drive,
        // edge detection enum values.
        let mut builder = Request::builder();
        builder.on_chip(&simc.dev_path).with_line(offset);

        let req = builder
            .as_active_low()
            .with_bias(gpiocdev::line::Bias::PullDown)
            .with_drive(gpiocdev::line::Drive::OpenDrain)
            .request()
            .unwrap();

        let cdevc = new_chip(&simc.dev_path, abiv);
        let info = cdevc.line_info(offset).unwrap();
        assert!(info.active_low);
        assert_eq!(info.direction, gpiocdev::line::Direction::Output);
        assert_eq!(info.bias, Some(gpiocdev::line::Bias::PullDown));
        assert_eq!(info.drive, Some(gpiocdev::line::Drive::OpenDrain));

        let mut cfg = req.config();
        cfg.with_bias(gpiocdev::line::Bias::PullUp)
            .with_drive(gpiocdev::line::Drive::OpenSource)
            .as_active_high();
        req.reconfigure(&cfg).unwrap();
        let info = cdevc.line_info(offset).unwrap();
        assert!(!info.active_low);
        assert_eq!(info.direction, gpiocdev::line::Direction::Output);
        assert_eq!(info.bias, Some(gpiocdev::line::Bias::PullUp));
        assert_eq!(info.drive, Some(gpiocdev::line::Drive::OpenSource));

        cfg.with_bias(gpiocdev::line::Bias::Disabled)
            .with_drive(gpiocdev::line::Drive::PushPull);
        req.reconfigure(&cfg).unwrap();
        let info = cdevc.line_info(offset).unwrap();
        assert!(!info.active_low);
        assert_eq!(info.direction, gpiocdev::line::Direction::Output);
        assert_eq!(info.bias, Some(gpiocdev::line::Bias::Disabled));
        assert_eq!(info.drive, Some(gpiocdev::line::Drive::PushPull));
        assert_eq!(info.edge_detection, None);
        assert_eq!(info.event_clock, None);
        assert_eq!(info.debounce_period, None);

        // can't reconfigure edges with v1, so re-request
        drop(req);
        cfg.with_edge_detection(gpiocdev::line::EdgeDetection::RisingEdge);
        let req = builder.with_config(cfg.clone()).request().unwrap();
        let info = cdevc.line_info(offset).unwrap();
        assert_eq!(info.direction, gpiocdev::line::Direction::Input);
        if abiv == gpiocdev::AbiVersion::V1 {
            assert_eq!(info.edge_detection, None);
            assert_eq!(info.event_clock, None);
        } else {
            assert_eq!(
                info.edge_detection,
                Some(gpiocdev::line::EdgeDetection::RisingEdge)
            );
            assert_eq!(
                info.event_clock,
                Some(gpiocdev::line::EventClock::Monotonic)
            );
        }

        drop(req);
        cfg.with_edge_detection(gpiocdev::line::EdgeDetection::FallingEdge);
        let req = builder.with_config(cfg.clone()).request().unwrap();
        let info = cdevc.line_info(offset).unwrap();
        assert_eq!(info.direction, gpiocdev::line::Direction::Input);
        if abiv == gpiocdev::AbiVersion::V1 {
            assert_eq!(info.edge_detection, None);
            assert_eq!(info.event_clock, None);
        } else {
            assert_eq!(
                info.edge_detection,
                Some(gpiocdev::line::EdgeDetection::FallingEdge)
            );
            assert_eq!(
                info.event_clock,
                Some(gpiocdev::line::EventClock::Monotonic)
            );
        }

        drop(req);
        cfg.with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges);
        let req = builder.with_config(cfg.clone()).request().unwrap();
        let info = cdevc.line_info(offset).unwrap();
        assert_eq!(info.direction, gpiocdev::line::Direction::Input);
        if abiv == gpiocdev::AbiVersion::V1 {
            assert_eq!(info.edge_detection, None);
            assert_eq!(info.event_clock, None);
        } else {
            assert_eq!(
                info.edge_detection,
                Some(gpiocdev::line::EdgeDetection::BothEdges)
            );
            assert_eq!(
                info.event_clock,
                Some(gpiocdev::line::EventClock::Monotonic)
            );
        }

        drop(req);
        if abiv == gpiocdev::AbiVersion::V2 {
            let req = builder
                .with_config(gpiocdev::request::Config::default())
                .with_line(0)
                .with_edge_detection(gpiocdev::line::EdgeDetection::FallingEdge)
                .with_event_clock(gpiocdev::line::EventClock::Realtime)
                .with_debounce_period(Duration::from_millis(10))
                .with_line(1)
                .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
                // .with_event_clock(gpiocdev::line::EventClock::HTE)
                // would be nice, but requires a kernel with HTE support...
                .request()
                .unwrap();

            let cdevc = Chip::from_path(&simc.dev_path).unwrap();
            let info = cdevc.line_info(0).unwrap();
            assert_eq!(info.direction, gpiocdev::line::Direction::Input);
            assert_eq!(
                info.edge_detection,
                Some(gpiocdev::line::EdgeDetection::FallingEdge)
            );
            assert_eq!(info.drive, None);
            assert_eq!(info.event_clock, Some(gpiocdev::line::EventClock::Realtime));
            assert_eq!(info.debounce_period, Some(Duration::from_millis(10)));

            let info = cdevc.line_info(1).unwrap();
            assert_eq!(info.direction, gpiocdev::line::Direction::Input);
            assert_eq!(
                info.edge_detection,
                Some(gpiocdev::line::EdgeDetection::BothEdges)
            );
            assert_eq!(info.drive, None);
            assert_eq!(
                info.event_clock,
                Some(gpiocdev::line::EventClock::Monotonic)
            );
            assert_eq!(info.debounce_period, None);

            drop(req);
        }
    }

    #[cfg(feature = "uapi_v1")]
    mod uapi_v1 {
        use gpiocdev::AbiVersion::V1;

        #[test]
        fn line_info() {
            super::line_info(V1);
        }

        #[test]
        fn watch_line_info() {
            super::watch_line_info(V1);
        }

        #[test]
        fn has_line_info_change_event() {
            super::has_line_info_change_event(V1);
        }

        #[test]
        fn read_line_info_change_event() {
            super::read_line_info_change_event(V1);
        }

        #[test]
        fn info_change_events() {
            super::info_change_events(V1);
        }

        #[test]
        fn wait_info_change_event() {
            super::wait_info_change_event(V1);
        }
    }

    #[cfg(feature = "uapi_v2")]
    mod uapi_v2 {
        use gpiocdev::AbiVersion::V2;

        #[test]
        fn line_info() {
            super::line_info(V2);
        }

        #[test]
        fn watch_line_info() {
            super::watch_line_info(V2);
        }

        #[test]
        fn has_line_info_change_event() {
            super::has_line_info_change_event(V2);
        }

        #[test]
        fn read_line_info_change_event() {
            super::read_line_info_change_event(V2);
        }

        #[test]
        fn info_change_events() {
            super::info_change_events(V2);
        }

        #[test]
        fn wait_info_change_event() {
            super::wait_info_change_event(V2);
        }
    }

    fn watch_line_info(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let cdevc = new_chip(&simc.dev_path, abiv);
        let offset = 2;

        let req = Request::builder()
            .on_chip(&simc.dev_path)
            .with_consumer("watch info")
            .with_line(offset)
            .as_input()
            .with_bias(gpiocdev::line::Bias::PullUp)
            .request()
            .unwrap();

        let xinfo = gpiocdev::line::Info {
            offset,
            bias: Some(gpiocdev::line::Bias::PullUp),
            consumer: "watch info".into(),
            used: true,
            ..Default::default()
        };
        let res = cdevc.watch_line_info(offset);
        assert_eq!(res, Ok(xinfo));
        assert_eq!(cdevc.has_line_info_change_event(), Ok(false));

        drop(req);
        assert_eq!(cdevc.has_line_info_change_event(), Ok(true));
    }

    #[test]
    fn unwatch_line_info() {
        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let cdevc = Chip::from_path(&simc.dev_path).unwrap();
        let offset = 3;

        let req = Request::builder()
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .request()
            .unwrap();

        assert!(cdevc.watch_line_info(offset).is_ok());
        assert_eq!(cdevc.has_line_info_change_event(), Ok(false));

        drop(req);
        assert_eq!(cdevc.has_line_info_change_event(), Ok(true));
        _ = cdevc.read_line_info_change_event();

        assert!(cdevc.unwatch_line_info(offset).is_ok());
        let req = Request::builder()
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .request()
            .unwrap();
        assert_eq!(cdevc.has_line_info_change_event(), Ok(false));

        drop(req);
        assert_eq!(cdevc.has_line_info_change_event(), Ok(false));
    }

    fn has_line_info_change_event(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let cdevc = new_chip(&simc.dev_path, abiv);

        for offset in 0..simc.cfg.num_lines {
            assert_eq!(cdevc.has_line_info_change_event(), Ok(false));
            assert!(cdevc.watch_line_info(offset).is_ok());
            assert_eq!(cdevc.has_line_info_change_event(), Ok(false));

            let req = Request::builder()
                .on_chip(&simc.dev_path)
                .with_line(offset)
                .request()
                .unwrap();
            assert_eq!(cdevc.has_line_info_change_event(), Ok(true));
            _ = cdevc.read_line_info_change_event();
            assert_eq!(cdevc.has_line_info_change_event(), Ok(false));

            drop(req);
            assert_eq!(cdevc.has_line_info_change_event(), Ok(true));
            _ = cdevc.read_line_info_change_event();
            assert_eq!(cdevc.has_line_info_change_event(), Ok(false));
        }
    }

    fn read_line_info_change_event(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let cdevc = new_chip(&simc.dev_path, abiv);

        for offset in 0..simc.cfg.num_lines {
            assert_eq!(cdevc.has_line_info_change_event(), Ok(false));
            assert!(cdevc.watch_line_info(offset).is_ok());
            assert_eq!(cdevc.has_line_info_change_event(), Ok(false));

            // request
            let req = Request::builder()
                .on_chip(&simc.dev_path)
                .with_line(offset)
                .as_input()
                .request()
                .unwrap();
            assert_eq!(cdevc.has_line_info_change_event(), Ok(true));
            let evt = cdevc.read_line_info_change_event().unwrap();
            assert_eq!(evt.kind, gpiocdev::line::InfoChangeKind::Requested);
            assert_eq!(evt.info.offset, offset);
            assert_eq!(evt.info.direction, gpiocdev::line::Direction::Input);
            assert_eq!(evt.info.edge_detection, None);
            assert_eq!(evt.info.edge_detection, None);
            assert_eq!(evt.info.debounce_period, None);

            // reconfigure
            let mut cfg = req.config();
            cfg.with_edge_detection(gpiocdev::line::EdgeDetection::RisingEdge)
                .with_debounce_period(Duration::from_millis(10));
            req.reconfigure(&cfg).unwrap();
            assert_eq!(cdevc.has_line_info_change_event(), Ok(true));
            let evt = cdevc.read_line_info_change_event().unwrap();
            assert_eq!(evt.kind, gpiocdev::line::InfoChangeKind::Reconfigured);
            assert_eq!(evt.info.offset, offset);
            assert_eq!(evt.info.direction, gpiocdev::line::Direction::Input);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(
                    evt.info.edge_detection,
                    Some(gpiocdev::line::EdgeDetection::RisingEdge)
                );
                assert_eq!(evt.info.debounce_period, Some(Duration::from_millis(10)));
            } else {
                assert_eq!(evt.info.edge_detection, None);
                assert_eq!(evt.info.debounce_period, None);
            }

            // release
            drop(req);
            let evt = cdevc.read_line_info_change_event().unwrap();
            assert_eq!(evt.kind, gpiocdev::line::InfoChangeKind::Released);
            assert_eq!(evt.info.offset, offset);
            assert_eq!(evt.info.edge_detection, None);
            assert_eq!(evt.info.debounce_period, None);
        }
    }

    fn info_change_events(abiv: gpiocdev::AbiVersion) {
        use gpiocdev::line::InfoChangeKind;
        use std::thread;

        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let cdevc = new_chip(&simc.dev_path, abiv);
        let chip_path = simc.dev_path.clone();
        let offset = 0;

        assert!(cdevc.watch_line_info(offset).is_ok());

        let t = thread::spawn(move || {
            // allow time for the iter to block
            thread::sleep(Duration::from_millis(10));

            // request
            let req = Request::builder()
                .on_chip(chip_path)
                .with_line(offset)
                .as_input()
                .request()
                .unwrap();

            // reconfigure
            let mut cfg = req.config();
            cfg.with_bias(gpiocdev::line::Bias::PullUp);
            req.reconfigure(&cfg).unwrap();

            // release
            drop(req);
        });
        let mut count = 0;
        let iter = cdevc.info_change_events().unwrap();
        for res in iter {
            assert!(res.is_ok());
            if let Ok(evt) = res {
                assert_eq!(evt.info.offset, offset);
                if count == 0 {
                    assert_eq!(evt.kind, InfoChangeKind::Requested);
                } else if count == 1 {
                    assert_eq!(evt.kind, InfoChangeKind::Reconfigured);
                } else {
                    assert_eq!(evt.kind, InfoChangeKind::Released);
                    break;
                }
                count += 1;
            }
        }

        let res = t.join();
        assert!(res.is_ok());
    }

    fn wait_info_change_event(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let cdevc = new_chip(&simc.dev_path, abiv);
        let offset = 0;

        assert!(cdevc.watch_line_info(offset).is_ok());
        assert_eq!(
            cdevc.wait_line_info_change_event(Duration::from_millis(1)),
            Ok(false)
        );

        let req = Request::builder()
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .request()
            .unwrap();

        assert_eq!(
            cdevc.wait_line_info_change_event(Duration::from_millis(1)),
            Ok(true)
        );

        _ = cdevc.read_line_info_change_event();
        assert_eq!(
            cdevc.wait_line_info_change_event(Duration::from_millis(1)),
            Ok(false)
        );

        drop(req);
        assert_eq!(
            cdevc.wait_line_info_change_event(Duration::from_millis(1)),
            Ok(true)
        );
    }

    #[test]
    fn detect_abi_version() {
        // assumes a kernel with both v1 and v2 supported.
        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let cdevc = Chip::from_path(&simc.dev_path).unwrap();
        #[cfg(feature = "uapi_v2")]
        assert_eq!(cdevc.detect_abi_version(), Ok(gpiocdev::AbiVersion::V2));
        #[cfg(not(feature = "uapi_v2"))]
        assert_eq!(cdevc.detect_abi_version(), Ok(gpiocdev::AbiVersion::V1));
    }

    #[test]
    fn supports_abi_version() {
        #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
        use gpiocdev::{AbiSupportKind, AbiVersion, Error};

        // assumes a kernel with both v1 and v2 supported.
        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let cdevc = Chip::from_path(&simc.dev_path).unwrap();
        #[cfg(feature = "uapi_v1")]
        assert_eq!(cdevc.supports_abi_version(gpiocdev::AbiVersion::V1), Ok(()));
        #[cfg(not(feature = "uapi_v1"))]
        assert_eq!(
            cdevc.supports_abi_version(gpiocdev::AbiVersion::V1),
            Err(Error::UnsupportedAbi(
                AbiVersion::V1,
                AbiSupportKind::Library
            ))
        );
        #[cfg(feature = "uapi_v2")]
        assert_eq!(cdevc.supports_abi_version(gpiocdev::AbiVersion::V2), Ok(()));
        #[cfg(not(feature = "uapi_v2"))]
        assert_eq!(
            cdevc.supports_abi_version(gpiocdev::AbiVersion::V2),
            Err(Error::UnsupportedAbi(
                AbiVersion::V2,
                AbiSupportKind::Library
            ))
        );
    }

    #[test]
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn using_abi_version() {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 2;

        let mut cdevc = Chip::from_path(&simc.dev_path).unwrap();

        let req = Request::builder()
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .with_event_clock(gpiocdev::line::EventClock::Realtime)
            .request()
            .unwrap();

        // defaults to v2
        let info_v2 = cdevc.line_info(offset).unwrap();
        assert_eq!(info_v2.direction, gpiocdev::line::Direction::Input);
        assert_eq!(
            info_v2.edge_detection,
            Some(gpiocdev::line::EdgeDetection::BothEdges)
        );
        assert_eq!(
            info_v2.event_clock,
            Some(gpiocdev::line::EventClock::Realtime)
        );

        // using v1
        cdevc.using_abi_version(gpiocdev::AbiVersion::V1);
        let info_v1 = cdevc.line_info(offset).unwrap();
        assert_eq!(info_v1.offset, offset);
        assert_eq!(info_v1.direction, gpiocdev::line::Direction::Input);
        // v1 does not report edge detection, event clock or debounce period
        // so this demonstrates v1 was used.
        assert_eq!(info_v1.edge_detection, None);
        assert_eq!(info_v1.event_clock, None);

        // using v2 again
        cdevc.using_abi_version(gpiocdev::AbiVersion::V2);

        // using v2 again
        let info_v2b = cdevc.line_info(offset).unwrap();
        assert_eq!(info_v2, info_v2b);

        drop(req);
    }

    #[allow(unused)]
    fn new_chip(path: &Path, abiv: gpiocdev::AbiVersion) -> gpiocdev::chip::Chip {
        let mut cdevc = Chip::from_path(path).unwrap();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        cdevc.using_abi_version(abiv);
        cdevc
    }
}
