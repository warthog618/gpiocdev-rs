// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
        assert_eq!(
            gpiocdev::chip::is_chip(c.dev_path()).unwrap(),
            *c.dev_path()
        );
    }
}

#[test]
fn is_chip_symlink() {
    let cc = bag_of_chips();
    let mut path = PathBuf::from("/tmp");
    path.push(gpiosim::unique_name("gpiocdev_chip", None));
    let link = Symlink::new(cc.chips()[0].dev_path(), &path).unwrap();
    // locates and reports the path of the actual device
    assert_eq!(*gpiocdev::chip::is_chip(&link.src).unwrap(), *link.dst);
}

#[test]
fn is_chip_nonexistent() {
    let path = PathBuf::from("/dev/gpiochip_nonexistent");
    assert_eq!(
        gpiocdev::chip::is_chip(path),
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
    let system_chips = gpiocdev::chip::chips().unwrap();
    for c in cc.chips() {
        // all chips in the test set must be in the system
        assert!(
            system_chips.contains(c.dev_path()),
            "{:?} not found in system chips",
            c.dev_path()
        );
    }
}

mod chip {
    use super::*;
    use gpiocdev::chip::Chip;
    use gpiocdev::request::Request;
    use gpiocdev::Error::GpioChip as ChipError;
    use gpiosim::{Bank, Direction, Simpleton};
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
        let s = bag_of_chips();
        for c in s.chips() {
            assert!(
                Chip::from_path(c.dev_path()).is_ok(),
                "failed to open {:?}",
                c.dev_path()
            );
        }
    }

    #[test]
    fn from_path_symlink() {
        let cc = bag_of_chips();
        let mut path = PathBuf::from("/tmp");
        path.push(gpiosim::unique_name("gpiocdev_chip", None));
        let link = Symlink::new(cc.chips()[0].dev_path(), &path).unwrap();
        // constructs and reports the path of the actual device
        assert_eq!(Chip::from_path(&link.src).unwrap().path(), link.dst);
    }

    #[test]
    fn from_path_nonexistent() {
        let path = PathBuf::from("/dev/gpiochip_nonexistent");
        assert_eq!(
            Chip::from_path(path).unwrap_err(),
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
        let s = bag_of_chips();
        for sc in s.chips() {
            let c = Chip::from_path(sc.dev_path()).unwrap();
            let info = c.info().unwrap();
            assert_eq!(info.num_lines, sc.config().num_lines);
            assert_eq!(info.label.as_str(), sc.config().label);
            assert_eq!(info.name.as_str(), sc.chip_name);
        }
    }

    #[test]
    fn name() {
        let s = bag_of_chips();
        for sc in s.chips() {
            let ch = Chip::from_path(sc.dev_path()).unwrap();
            assert_eq!(ch.name().as_str(), sc.chip_name);
        }
    }

    #[test]
    fn path() {
        let s = bag_of_chips();
        for sc in s.chips() {
            let c = Chip::from_path(sc.dev_path()).unwrap();
            assert_eq!(c.path(), sc.dev_path());
        }
    }

    #[test]
    fn find_line_info() {
        let s = detailed_sim();
        for sc in s.chips() {
            let c = Chip::from_path(sc.dev_path()).unwrap();
            for (offset, name) in &sc.config().names {
                let info = c.find_line_info(name).unwrap();
                assert_eq!(info.offset, *offset);
                assert_eq!(info.name, *name);
            }

            // non-existent
            assert!(c.find_line_info("not such line").is_none())
        }
    }

    fn line_info(abiv: gpiocdev::AbiVersion) {
        let s = detailed_sim();
        for sc in s.chips() {
            let c = new_chip(sc.dev_path(), abiv);
            // names, offsets and used
            for (offset, name) in &sc.config().names {
                let info = c.line_info(*offset).unwrap();
                assert_eq!(info.name.as_str(), name);
                assert_eq!(info.offset, *offset);
                assert_eq!(info.used, sc.config().hogs.contains_key(offset))
            }

            // consumer, direction and used
            for (offset, hog) in &sc.config().hogs {
                let info = c.line_info(*offset).unwrap();
                assert_eq!(info.consumer.as_str(), &hog.consumer);
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
        let s = Simpleton::new(10);
        let offset = 1;
        // Combinations covering all possible active low, bias, drive,
        // edge detection enum values.
        let mut builder = Request::builder();
        builder.on_chip(s.dev_path()).with_line(offset);

        let c = new_chip(s.dev_path(), abiv);
        let res = c.line_info(11);
        assert_eq!(
            res,
            Err(gpiocdev::Error::UapiError(
                gpiocdev::UapiCall::GetLineInfo,
                gpiocdev_uapi::Error::Os(Errno(22))
            ))
        );

        let req = builder
            .as_active_low()
            .with_bias(gpiocdev::line::Bias::PullDown)
            .with_drive(gpiocdev::line::Drive::OpenDrain)
            .request()
            .unwrap();

        let info = c.line_info(offset).unwrap();
        assert!(info.active_low);
        assert_eq!(info.direction, gpiocdev::line::Direction::Output);
        assert_eq!(info.bias, Some(gpiocdev::line::Bias::PullDown));
        assert_eq!(info.drive, Some(gpiocdev::line::Drive::OpenDrain));

        let mut cfg = req.config();
        cfg.with_bias(gpiocdev::line::Bias::PullUp)
            .with_drive(gpiocdev::line::Drive::OpenSource)
            .as_active_high();
        req.reconfigure(&cfg).unwrap();
        let info = c.line_info(offset).unwrap();
        assert!(!info.active_low);
        assert_eq!(info.direction, gpiocdev::line::Direction::Output);
        assert_eq!(info.bias, Some(gpiocdev::line::Bias::PullUp));
        assert_eq!(info.drive, Some(gpiocdev::line::Drive::OpenSource));

        cfg.with_bias(gpiocdev::line::Bias::Disabled)
            .with_drive(gpiocdev::line::Drive::PushPull);
        req.reconfigure(&cfg).unwrap();
        let info = c.line_info(offset).unwrap();
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
        let info = c.line_info(offset).unwrap();
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
        let info = c.line_info(offset).unwrap();
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
        let info = c.line_info(offset).unwrap();
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
                .on_chip(s.dev_path())
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

            let c = Chip::from_path(s.dev_path()).unwrap();
            let info = c.line_info(0).unwrap();
            assert_eq!(info.direction, gpiocdev::line::Direction::Input);
            assert_eq!(
                info.edge_detection,
                Some(gpiocdev::line::EdgeDetection::FallingEdge)
            );
            assert_eq!(info.drive, None);
            assert_eq!(info.event_clock, Some(gpiocdev::line::EventClock::Realtime));
            assert_eq!(info.debounce_period, Some(Duration::from_millis(10)));

            let info = c.line_info(1).unwrap();
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
        let s = Simpleton::new(4);
        let c = new_chip(s.dev_path(), abiv);
        let offset = 2;

        assert_eq!(
            c.watch_line_info(5),
            Err(gpiocdev::Error::UapiError(
                gpiocdev::UapiCall::WatchLineInfo,
                gpiocdev_uapi::Error::Os(Errno(22))
            ))
        );

        let req = Request::builder()
            .on_chip(s.dev_path())
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
        let res = c.watch_line_info(offset);
        assert_eq!(res, Ok(xinfo));
        assert_eq!(c.has_line_info_change_event(), Ok(false));

        drop(req);
        assert_eq!(c.has_line_info_change_event(), Ok(true));
    }

    #[test]
    fn unwatch_line_info() {
        let s = Simpleton::new(4);
        let c = Chip::from_path(s.dev_path()).unwrap();
        let offset = 3;

        assert_eq!(
            c.unwatch_line_info(5),
            Err(gpiocdev::Error::UapiError(
                gpiocdev::UapiCall::UnwatchLineInfo,
                gpiocdev_uapi::Error::Os(Errno(22))
            ))
        );

        let req = Request::builder()
            .on_chip(s.dev_path())
            .with_line(offset)
            .request()
            .unwrap();

        assert!(c.watch_line_info(offset).is_ok());
        assert_eq!(c.has_line_info_change_event(), Ok(false));

        drop(req);
        assert_eq!(c.has_line_info_change_event(), Ok(true));
        _ = c.read_line_info_change_event();

        assert!(c.unwatch_line_info(offset).is_ok());
        let req = Request::builder()
            .on_chip(s.dev_path())
            .with_line(offset)
            .request()
            .unwrap();
        assert_eq!(c.has_line_info_change_event(), Ok(false));

        drop(req);
        assert_eq!(c.has_line_info_change_event(), Ok(false));
    }

    fn has_line_info_change_event(abiv: gpiocdev::AbiVersion) {
        let s = Simpleton::new(4);
        let c = new_chip(s.dev_path(), abiv);

        for offset in 0..s.config().num_lines {
            assert_eq!(c.has_line_info_change_event(), Ok(false));
            assert!(c.watch_line_info(offset).is_ok());
            assert_eq!(c.has_line_info_change_event(), Ok(false));

            let req = Request::builder()
                .on_chip(s.dev_path())
                .with_line(offset)
                .request()
                .unwrap();
            assert_eq!(c.has_line_info_change_event(), Ok(true));
            _ = c.read_line_info_change_event();
            assert_eq!(c.has_line_info_change_event(), Ok(false));

            drop(req);
            assert_eq!(c.has_line_info_change_event(), Ok(true));
            _ = c.read_line_info_change_event();
            assert_eq!(c.has_line_info_change_event(), Ok(false));
        }
    }

    fn read_line_info_change_event(abiv: gpiocdev::AbiVersion) {
        let s = Simpleton::new(4);
        let c = new_chip(s.dev_path(), abiv);

        for offset in 0..s.config().num_lines {
            assert_eq!(c.has_line_info_change_event(), Ok(false));
            assert!(c.watch_line_info(offset).is_ok());
            assert_eq!(c.has_line_info_change_event(), Ok(false));

            // request
            let req = Request::builder()
                .on_chip(s.dev_path())
                .with_line(offset)
                .as_input()
                .request()
                .unwrap();
            assert_eq!(c.has_line_info_change_event(), Ok(true));
            let evt = c.read_line_info_change_event().unwrap();
            assert_eq!(evt.kind, gpiocdev::line::InfoChangeKind::Requested);
            assert_eq!(evt.info.offset, offset);
            assert_eq!(evt.info.direction, gpiocdev::line::Direction::Input);
            assert_eq!(evt.info.edge_detection, None);
            assert_eq!(evt.info.edge_detection, None);
            assert_eq!(evt.info.debounce_period, None);

            // reconfigure
            let mut cfg = req.config();
            cfg.with_bias(gpiocdev::line::Bias::PullUp);
            if abiv == gpiocdev::AbiVersion::V2 {
                cfg.with_edge_detection(gpiocdev::line::EdgeDetection::RisingEdge)
                    .with_debounce_period(Duration::from_millis(10));
            }
            req.reconfigure(&cfg).unwrap();
            assert_eq!(c.has_line_info_change_event(), Ok(true));
            let evt = c.read_line_info_change_event().unwrap();
            assert_eq!(evt.kind, gpiocdev::line::InfoChangeKind::Reconfigured);
            assert_eq!(evt.info.offset, offset);
            assert_eq!(evt.info.direction, gpiocdev::line::Direction::Input);
            assert_eq!(evt.info.bias, Some(gpiocdev::line::Bias::PullUp));
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
            let evt = c.read_line_info_change_event().unwrap();
            assert_eq!(evt.kind, gpiocdev::line::InfoChangeKind::Released);
            assert_eq!(evt.info.offset, offset);
            assert_eq!(evt.info.edge_detection, None);
            assert_eq!(evt.info.debounce_period, None);
        }
    }

    fn info_change_events(abiv: gpiocdev::AbiVersion) {
        use gpiocdev::line::{Bias, InfoChangeKind};
        use std::sync::mpsc;
        use std::thread;
        let s = Simpleton::new(4);
        let c = new_chip(s.dev_path(), abiv);
        let offset = 0;

        let (bg_tx, bg_rx) = mpsc::channel();
        assert!(c.watch_line_info(offset).is_ok());
        let t = thread::spawn(move || {
            // request
            let req = Request::builder()
                .on_chip(s.dev_path())
                .with_line(offset)
                .as_input()
                .request()
                .unwrap();

            // reconfigure pullup
            bg_rx.recv().unwrap();
            let mut cfg = req.config();
            cfg.with_bias(Bias::PullUp);
            req.reconfigure(&cfg).unwrap();

            // reconfigure pulldown
            bg_rx.recv().unwrap();
            cfg.with_bias(Bias::PullDown);
            req.reconfigure(&cfg).unwrap();

            bg_rx.recv().unwrap();
            drop(req);
        });
        let mut count = 0;
        for res in c.info_change_events() {
            assert!(res.is_ok());
            if let Ok(evt) = res {
                assert_eq!(evt.info.offset, offset);
                match count {
                    0 => {
                        assert_eq!(evt.kind, InfoChangeKind::Requested);
                        assert_eq!(evt.info.bias, None);
                    }
                    1 => {
                        assert_eq!(evt.kind, InfoChangeKind::Reconfigured);
                        assert_eq!(evt.info.bias, Some(Bias::PullUp));
                    }
                    2 => {
                        assert_eq!(evt.kind, InfoChangeKind::Reconfigured);
                        assert_eq!(evt.info.bias, Some(Bias::PullDown));
                    }
                    _ => {
                        assert_eq!(evt.kind, InfoChangeKind::Released);
                        assert_eq!(evt.info.bias, None);
                        break;
                    }
                }
                count += 1;
                // kick the bg thread
                bg_tx.send(1).unwrap();
            }
        }
        let res = t.join();
        assert!(res.is_ok());
    }

    fn wait_info_change_event(abiv: gpiocdev::AbiVersion) {
        let s = Simpleton::new(4);
        let c = new_chip(s.dev_path(), abiv);
        let offset = 0;

        assert!(c.watch_line_info(offset).is_ok());
        assert_eq!(
            c.wait_line_info_change_event(Duration::from_millis(1)),
            Ok(false)
        );

        let req = Request::builder()
            .on_chip(s.dev_path())
            .with_line(offset)
            .request()
            .unwrap();

        assert_eq!(
            c.wait_line_info_change_event(Duration::from_millis(1)),
            Ok(true)
        );

        _ = c.read_line_info_change_event();
        assert_eq!(
            c.wait_line_info_change_event(Duration::from_millis(1)),
            Ok(false)
        );

        drop(req);
        assert_eq!(
            c.wait_line_info_change_event(Duration::from_millis(1)),
            Ok(true)
        );
    }

    #[test]
    fn detect_abi_version() {
        // assumes a kernel with both v1 and v2 supported.
        let s = Simpleton::new(4);
        let c = Chip::from_path(s.dev_path()).unwrap();
        #[cfg(feature = "uapi_v2")]
        assert_eq!(c.detect_abi_version(), Ok(gpiocdev::AbiVersion::V2));
        #[cfg(not(feature = "uapi_v2"))]
        assert_eq!(c.detect_abi_version(), Ok(gpiocdev::AbiVersion::V1));
    }

    #[test]
    fn supports_abi_version() {
        // assumes a kernel with both v1 and v2 supported.
        let s = Simpleton::new(4);
        let c = Chip::from_path(s.dev_path()).unwrap();
        #[cfg(feature = "uapi_v1")]
        assert_eq!(c.supports_abi_version(gpiocdev::AbiVersion::V1), Ok(()));
        #[cfg(not(feature = "uapi_v1"))]
        assert_eq!(
            c.supports_abi_version(gpiocdev::AbiVersion::V1),
            Err(gpiocdev::Error::UnsupportedAbi(
                gpiocdev::AbiVersion::V1,
                gpiocdev::AbiSupportKind::Build
            ))
        );
        #[cfg(feature = "uapi_v2")]
        assert_eq!(c.supports_abi_version(gpiocdev::AbiVersion::V2), Ok(()));
        #[cfg(not(feature = "uapi_v2"))]
        assert_eq!(
            c.supports_abi_version(gpiocdev::AbiVersion::V2),
            Err(gpiocdev::Error::UnsupportedAbi(
                gpiocdev::AbiVersion::V2,
                gpiocdev::AbiSupportKind::Build
            ))
        );
    }

    #[test]
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn using_abi_version() {
        let s = Simpleton::new(3);
        let offset = 2;

        let mut c = Chip::from_path(s.dev_path()).unwrap();

        let req = Request::builder()
            .on_chip(s.dev_path())
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .with_event_clock(gpiocdev::line::EventClock::Realtime)
            .request()
            .unwrap();

        // defaults to v2
        let info_v2 = c.line_info(offset).unwrap();
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
        c.using_abi_version(gpiocdev::AbiVersion::V1);
        let info_v1 = c.line_info(offset).unwrap();
        assert_eq!(info_v1.offset, offset);
        assert_eq!(info_v1.direction, gpiocdev::line::Direction::Input);
        // v1 does not report edge detection, event clock or debounce period
        // so this demonstrates v1 was used.
        assert_eq!(info_v1.edge_detection, None);
        assert_eq!(info_v1.event_clock, None);

        // using v2 again
        c.using_abi_version(gpiocdev::AbiVersion::V2);

        // using v2 again
        let info_v2b = c.line_info(offset).unwrap();
        assert_eq!(info_v2, info_v2b);

        drop(req);
    }

    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn new_chip(path: &Path, abiv: gpiocdev::AbiVersion) -> gpiocdev::chip::Chip {
        let mut c = Chip::from_path(path).unwrap();
        c.using_abi_version(abiv);
        c
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn new_chip(path: &Path, _abiv: gpiocdev::AbiVersion) -> gpiocdev::chip::Chip {
        Chip::from_path(path).unwrap()
    }
}
