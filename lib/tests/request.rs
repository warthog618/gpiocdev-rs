// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use gpiocdev::line::{EdgeKind, Value, Values};
use gpiocdev::request::Request;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

mod builder {
    use super::*;
    use errno::Errno;
    use gpiocdev::chip::{Chip, ErrorKind};
    use gpiocdev::AbiVersion;
    use gpiocdev::Error::GpioChip as ChipError;
    use gpiosim::Level;

    #[cfg(feature = "uapi_v1")]
    mod uapi_v1 {
        use gpiocdev::line::{EdgeDetection, EventClock};
        use gpiocdev::request::Request;
        use gpiocdev::AbiVersion::V1;
        use std::time::Duration;

        #[test]
        fn request() {
            super::request(V1)
        }

        #[test]
        fn request_mixed_config() {
            super::request_mixed_config(V1)
        }

        #[test]
        fn request_invalid_offset() {
            super::request_invalid_offset(V1)
        }

        #[test]
        fn request_debounced() {
            let sim = gpiosim::simpleton(10);

            let mut builder = Request::builder();
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            builder.using_abi_version(V1);

            let res = builder
                .on_chip(&sim.chip().dev_path)
                .with_line(1)
                .as_input()
                .with_edge_detection(EdgeDetection::BothEdges)
                .with_debounce_period(Duration::from_millis(4))
                .request();
            assert_eq!(
                res.unwrap_err(),
                gpiocdev::Error::AbiLimitation(V1, "does not support debounce".to_string(),)
            );
        }

        #[test]
        fn request_event_clock() {
            let sim = gpiosim::simpleton(10);

            let mut builder = Request::builder();
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            builder.using_abi_version(V1);

            let res = builder
                .on_chip(&sim.chip().dev_path)
                .with_line(1)
                .as_input()
                .with_edge_detection(EdgeDetection::BothEdges)
                .with_event_clock(EventClock::Realtime)
                .request();
            assert_eq!(
                res.unwrap_err(),
                gpiocdev::Error::AbiLimitation(
                    V1,
                    "does not support selecting the event clock source".to_string(),
                )
            );
        }

        #[test]
        fn request_kernel_event_buffer_size() {
            let sim = gpiosim::simpleton(10);

            let mut builder = Request::builder();
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            builder.using_abi_version(V1);

            let res = builder
                .on_chip(&sim.chip().dev_path)
                .with_line(1)
                .as_input()
                .with_edge_detection(EdgeDetection::BothEdges)
                .with_kernel_event_buffer_size(42)
                .request();
            assert_eq!(
                res.unwrap_err(),
                gpiocdev::Error::AbiLimitation(
                    V1,
                    "does not support setting event buffer size".to_string(),
                )
            );
        }
    }

    #[cfg(feature = "uapi_v2")]
    mod uapi_v2 {
        use gpiocdev::chip::Chip;
        use gpiocdev::request::Request;
        use gpiocdev::AbiVersion::V2;
        use std::time::Duration;

        #[test]
        fn request() {
            super::request(V2)
        }

        #[test]
        fn request_mixed_config() {
            super::request_mixed_config(V2)
        }

        #[test]
        fn request_invalid_offset() {
            super::request_invalid_offset(V2)
        }

        #[test]
        fn request_too_complicated() {
            let sim = gpiosim::simpleton(25);
            let simc = sim.chip();

            let mut cfg = gpiocdev::request::Config::default();
            for offset in 1..20 {
                cfg.with_line(offset)
                    .with_debounce_period(Duration::from_millis(offset as u64));
            }
            let res = Request::builder()
                .on_chip(&simc.dev_path)
                .with_config(cfg)
                .as_input()
                .request();
            assert_eq!(
                res.unwrap_err().to_string(),
                "uAPI ABI v2 supports 10 attrs, configuration requires 19."
            );
        }

        #[test]
        fn request_debounced() {
            use gpiocdev::line::{Direction, EdgeDetection, EventClock};

            let sim = gpiosim::simpleton(10);
            let simc = sim.chip();
            let cdevc = Chip::from_path(&simc.dev_path).unwrap();
            let offset = 1;

            let req = Request::builder()
                .on_chip(&simc.dev_path)
                .with_line(offset)
                .as_input()
                .with_edge_detection(EdgeDetection::BothEdges)
                .with_debounce_period(Duration::from_millis(4))
                .request()
                .unwrap();

            let info = cdevc.line_info(offset).unwrap();
            assert!(!info.active_low);
            assert_eq!(info.direction, Direction::Input);
            assert_eq!(info.edge_detection, Some(EdgeDetection::BothEdges));
            assert_eq!(info.debounce_period, Some(Duration::from_millis(4)));
            assert_eq!(info.event_clock, Some(EventClock::Monotonic));

            drop(req);
        }

        #[test]
        fn request_event_clock() {
            use gpiocdev::line::{Direction, EdgeDetection, EventClock};

            let sim = gpiosim::simpleton(10);
            let simc = sim.chip();
            let cdevc = Chip::from_path(&simc.dev_path).unwrap();
            let offset = 1;

            let mut builder = Request::builder();
            builder
                .on_chip(&simc.dev_path)
                .with_line(offset)
                .as_input()
                .with_edge_detection(EdgeDetection::BothEdges)
                .with_event_clock(EventClock::Realtime);

            let req = builder.request().unwrap();

            let info = cdevc.line_info(offset).unwrap();
            assert!(!info.active_low);
            assert_eq!(info.direction, Direction::Input);
            assert_eq!(info.edge_detection, Some(EdgeDetection::BothEdges));
            assert_eq!(info.event_clock, Some(EventClock::Realtime));

            drop(req);

            let res = builder.with_event_clock(EventClock::HTE).request();
            assert_eq!(
                res.unwrap_err(),
                gpiocdev::Error::UapiError(
                    gpiocdev::UapiCall::GetLine,
                    gpiocdev_uapi::Error::Os(errno::Errno(95))
                )
            );
        }

        #[test]
        fn request_kernel_event_buffer_size() {
            use gpiocdev::line::EdgeDetection;

            let sim = gpiosim::simpleton(10);

            let res = Request::builder()
                .on_chip(&sim.chip().dev_path)
                .with_line(1)
                .as_input()
                .with_edge_detection(EdgeDetection::BothEdges)
                .with_kernel_event_buffer_size(128)
                .request();

            assert!(res.is_ok());
            // a more complete test would be to generate events and overflow
            // the kernel buffer, but the size is only a hint, so the test would
            // have to make assumptions about kernel internals.
        }
    }

    fn request(abiv: gpiocdev::AbiVersion) {
        use gpiocdev::line::{Bias, Direction, Drive, EdgeDetection};

        // config menagerie on simpleton
        let sim = gpiosim::simpleton(10);
        let simc = sim.chip();
        let cdevc = Chip::from_path(&simc.dev_path).unwrap();
        let offset = 1;

        let mut builder = Request::builder();
        builder.on_chip(&simc.dev_path).with_line(offset);
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        // -- single line outputs, all field variants

        let req = builder.as_output(Value::Inactive).request().unwrap();

        let info = cdevc.line_info(offset).unwrap();
        assert!(!info.active_low);
        assert_eq!(info.direction, Direction::Output);
        assert_eq!(info.bias, None);
        assert_eq!(info.drive, Some(Drive::PushPull));
        assert_eq!(info.edge_detection, None);
        assert_eq!(info.debounce_period, None);
        assert_eq!(simc.get_level(offset).unwrap(), Level::Low);

        drop(req);
        let req = builder
            .as_active_low()
            .with_bias(gpiocdev::line::Bias::PullUp)
            .with_drive(gpiocdev::line::Drive::OpenDrain)
            .request()
            .unwrap();

        let info = cdevc.line_info(offset).unwrap();
        assert!(info.active_low);
        assert_eq!(info.direction, Direction::Output);
        assert_eq!(info.bias, Some(Bias::PullUp));
        assert_eq!(info.drive, Some(Drive::OpenDrain));
        assert_eq!(info.edge_detection, None);
        assert_eq!(info.debounce_period, None);
        assert_eq!(simc.get_level(offset).unwrap(), Level::High);

        drop(req);
        let req = builder
            .as_active_high()
            .with_bias(gpiocdev::line::Bias::PullDown)
            .with_drive(gpiocdev::line::Drive::OpenSource)
            .request()
            .unwrap();

        let info = cdevc.line_info(offset).unwrap();
        assert!(!info.active_low);
        assert_eq!(info.direction, Direction::Output);
        assert_eq!(info.bias, Some(Bias::PullDown));
        assert_eq!(info.drive, Some(Drive::OpenSource));
        assert_eq!(info.edge_detection, None);
        assert_eq!(info.debounce_period, None);
        assert_eq!(simc.get_level(offset).unwrap(), Level::Low);

        drop(req);
        let req = builder
            .with_bias(Bias::Disabled)
            .with_drive(Drive::PushPull)
            .as_output(Value::Active)
            .request()
            .unwrap();

        let info = cdevc.line_info(offset).unwrap();
        assert!(!info.active_low);
        assert_eq!(info.direction, Direction::Output);
        assert_eq!(info.bias, Some(Bias::Disabled));
        assert_eq!(info.drive, Some(Drive::PushPull));
        assert_eq!(info.edge_detection, None);
        assert_eq!(info.debounce_period, None);
        assert_eq!(simc.get_level(offset).unwrap(), Level::High);

        drop(req);

        // -- single line inputs, all field variants

        let mut builder = Request::builder();
        builder.on_chip(&simc.dev_path).with_line(offset).as_input();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = builder.request().unwrap();

        simc.set_pull(offset, Level::High).unwrap();
        propagation_delay();
        let info = cdevc.line_info(offset).unwrap();
        assert!(!info.active_low);
        assert_eq!(info.direction, Direction::Input);
        assert_eq!(info.bias, None);
        assert_eq!(info.drive, None);
        assert_eq!(info.edge_detection, None);
        assert_eq!(info.debounce_period, None);
        assert_eq!(req.value(offset).unwrap(), Value::Active);

        drop(req);
        let req = builder
            .as_active_low()
            .with_bias(Bias::PullUp)
            .with_edge_detection(EdgeDetection::RisingEdge)
            .request()
            .unwrap();

        let info = cdevc.line_info(offset).unwrap();
        assert!(info.active_low);
        assert_eq!(info.direction, Direction::Input);
        assert_eq!(info.bias, Some(Bias::PullUp));
        assert_eq!(info.drive, None);
        if abiv == AbiVersion::V2 {
            assert_eq!(info.edge_detection, Some(EdgeDetection::RisingEdge));
        } else {
            assert_eq!(info.edge_detection, None);
        }
        assert_eq!(req.value(offset).unwrap(), Value::Inactive);

        drop(req);
        let req = builder
            .as_active_high()
            .with_bias(Bias::PullDown)
            .with_edge_detection(EdgeDetection::FallingEdge)
            .request()
            .unwrap();

        let info = cdevc.line_info(offset).unwrap();
        assert!(!info.active_low);
        assert_eq!(info.direction, Direction::Input);
        assert_eq!(info.bias, Some(Bias::PullDown));
        assert_eq!(info.drive, None);
        if abiv == AbiVersion::V2 {
            assert_eq!(info.edge_detection, Some(EdgeDetection::FallingEdge));
        } else {
            assert_eq!(info.edge_detection, None);
        }
        assert_eq!(info.debounce_period, None);
        assert_eq!(req.value(offset).unwrap(), Value::Inactive);

        drop(req);
        let req = builder
            .with_bias(Bias::Disabled)
            .with_edge_detection(EdgeDetection::BothEdges)
            .request()
            .unwrap();

        let info = cdevc.line_info(offset).unwrap();
        assert!(!info.active_low);
        assert_eq!(info.direction, Direction::Input);
        assert_eq!(info.bias, Some(Bias::Disabled));
        assert_eq!(info.drive, None);
        if abiv == AbiVersion::V2 {
            assert_eq!(info.edge_detection, Some(EdgeDetection::BothEdges));
        } else {
            assert_eq!(info.edge_detection, None);
        }
        assert_eq!(info.debounce_period, None);
        assert_eq!(req.value(offset).unwrap(), Value::Inactive);

        drop(req);

        // -- multi-line output
        let offsets = &[2, 7];
        let mut builder = Request::builder();
        builder
            .on_chip(&simc.dev_path)
            .with_lines(offsets)
            .as_input();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);
        let req = builder.request().unwrap();
        for offset in offsets {
            let info = cdevc.line_info(*offset).unwrap();
            assert!(info.used);
            assert!(!info.active_low);
            assert_eq!(info.direction, Direction::Input);
            assert_eq!(info.bias, None);
            assert_eq!(info.drive, None);
            assert_eq!(info.edge_detection, None);
            assert_eq!(info.debounce_period, None);
        }

        drop(req);

        // -- multi-line input
        builder.as_output(Value::Active);
        let req = builder.request().unwrap();
        for offset in offsets {
            let info = cdevc.line_info(*offset).unwrap();
            assert!(info.used);
            assert!(!info.active_low);
            assert_eq!(info.direction, Direction::Output);
            assert_eq!(info.bias, None);
            assert_eq!(info.drive, Some(Drive::PushPull));
            assert_eq!(info.edge_detection, None);
            assert_eq!(info.debounce_period, None);
        }

        drop(req);
    }

    fn request_mixed_config(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();

        let mut builder = Request::builder();
        builder
            .on_chip(&simc.dev_path)
            .with_line(1)
            .as_input()
            .with_line(2)
            .as_output(Value::Active);
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);
        let res = builder.request();
        if abiv == AbiVersion::V1 {
            assert_eq!(
                res.unwrap_err().to_string(),
                "uAPI ABI v1 requires all lines to share the same configuration."
            );
        } else {
            assert!(res.is_ok());
        }
    }

    fn request_invalid_offset(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();

        let res = Request::builder()
            .on_chip(&simc.dev_path)
            .with_line(5)
            .as_input()
            .request();
        if abiv == AbiVersion::V2 {
            assert_eq!(
                res.unwrap_err(),
                gpiocdev::Error::UapiError(
                    gpiocdev::UapiCall::GetLine,
                    gpiocdev_uapi::Error::Os(Errno(22))
                )
            );
        } else {
            #[cfg(not(feature = "uapi_v2"))]
            assert_eq!(
                res.unwrap_err(),
                gpiocdev::Error::UapiError(
                    gpiocdev::UapiCall::GetLineHandle,
                    gpiocdev_uapi::Error::Os(Errno(22))
                )
            );
        }
    }

    #[test]
    fn request_symlink_chip() {
        let sim = gpiosim::simpleton(4);
        let mut path = PathBuf::from("/tmp");
        path.push(gpiosim::unique_name("gpiocdev_builder", None));
        let link = Symlink::new(&sim.chip().dev_path, &path).unwrap();
        let req = Request::builder()
            .on_chip(&link.src)
            .with_line(2)
            .as_input()
            .request();
        assert!(req.is_ok());
    }

    #[test]
    fn request_chip_nonexistent() {
        let res = Request::builder()
            .on_chip("/dev/nonexistent")
            .with_line(5)
            .as_input()
            .request();
        assert_eq!(res.unwrap_err(), gpiocdev::Error::from(Errno(2)));
    }

    #[test]
    fn request_chip_not_a_character_device() {
        let path = PathBuf::from("/tmp");
        let res = Request::builder()
            .on_chip(&path)
            .with_line(5)
            .as_input()
            .request();
        assert_eq!(
            res.unwrap_err(),
            ChipError(path, ErrorKind::NotCharacterDevice)
        );
    }

    #[test]
    fn request_chip_not_a_gpio_device() {
        let path = PathBuf::from("/dev/zero");
        let res = Request::builder()
            .on_chip(&path)
            .with_line(5)
            .as_input()
            .request();
        assert_eq!(res.unwrap_err(), ChipError(path, ErrorKind::NotGpioDevice));
    }
}

mod request {
    use super::*;

    #[cfg(feature = "uapi_v1")]
    mod uapi_v1 {
        use super::propagation_delay;
        use gpiocdev::request::Request;
        use gpiocdev::AbiVersion::V1;

        #[test]
        fn value() {
            super::value(V1);
        }

        #[test]
        fn values() {
            super::values(V1);
        }

        #[test]
        fn set_value() {
            super::set_value(V1);
        }

        #[test]
        fn set_values() {
            super::set_values(V1);
        }

        #[test]
        fn reconfigure() {
            super::reconfigure(V1);
        }

        #[test]
        fn has_edge_event() {
            super::has_edge_event(V1);
        }

        #[test]
        fn wait_edge_event() {
            super::wait_edge_event(V1);
        }

        #[test]
        fn read_edge_event() {
            super::read_edge_event(V1);
        }

        #[test]
        fn new_edge_event_buffer() {
            super::new_edge_event_buffer(V1);
        }

        #[test]
        fn edge_events() {
            let sim = gpiosim::simpleton(3);
            let simc = sim.chip();
            let offset = 2;

            let mut builder = Request::builder();
            builder
                .on_chip(&simc.dev_path)
                .with_line(offset)
                .as_input()
                .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges);
            #[cfg(feature = "uapi_v2")]
            builder.using_abi_version(gpiocdev::AbiVersion::V1);
            let req = builder.request().unwrap();

            // create four events
            simc.toggle(offset).unwrap();
            propagation_delay();
            simc.toggle(offset).unwrap();
            propagation_delay();
            simc.toggle(offset).unwrap();
            propagation_delay();
            simc.toggle(offset).unwrap();
            propagation_delay();

            let mut iter = req.edge_events().unwrap();

            let evt = iter.next().unwrap().unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            assert_eq!(evt.line_seqno, 0);
            assert_eq!(evt.seqno, 0);

            let evt = iter.next().unwrap().unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            assert_eq!(evt.line_seqno, 0);
            assert_eq!(evt.seqno, 0);

            let evt = iter.next().unwrap().unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            assert_eq!(evt.line_seqno, 0);
            assert_eq!(evt.seqno, 0);

            let evt = iter.next().unwrap().unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            assert_eq!(evt.line_seqno, 0);
            assert_eq!(evt.seqno, 0);
        }

        #[test]
        fn read_edge_events_into_slice() {
            super::read_edge_events_into_slice(gpiocdev::AbiVersion::V1)
        }

        #[test]
        fn edge_event_from_slice() {
            let sim = gpiosim::simpleton(3);
            let simc = sim.chip();
            let offset = 2;

            let mut builder = Request::builder();
            #[cfg(feature = "uapi_v2")]
            builder.using_abi_version(gpiocdev::AbiVersion::V1);
            let req = builder
                .on_chip(&simc.dev_path)
                .with_line(offset)
                .as_input()
                .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
                .request()
                .unwrap();
            let mut buf = vec![0; req.edge_event_size() * 3];

            // create four events
            simc.toggle(offset).unwrap();
            propagation_delay();
            simc.toggle(offset).unwrap();
            propagation_delay();
            simc.toggle(offset).unwrap();
            propagation_delay();
            simc.toggle(offset).unwrap();
            propagation_delay();

            // read a buffer full
            let wlen = req.read_edge_events_into_slice(buf.as_mut_slice()).unwrap();
            assert_eq!(wlen, buf.capacity());

            let evt = req.edge_event_from_slice(buf.as_slice()).unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            assert_eq!(evt.line_seqno, 0);
            assert_eq!(evt.seqno, 0);

            let evt = req
                .edge_event_from_slice(&buf.as_slice()[req.edge_event_size()..])
                .unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            assert_eq!(evt.line_seqno, 0);
            assert_eq!(evt.seqno, 0);

            let evt = req
                .edge_event_from_slice(&buf.as_slice()[req.edge_event_size() * 2..])
                .unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            assert_eq!(evt.line_seqno, 0);
            assert_eq!(evt.seqno, 0);

            // read remaining event
            let wlen = req.read_edge_events_into_slice(buf.as_mut_slice()).unwrap();
            assert_eq!(wlen, req.edge_event_size());

            let evt = req.edge_event_from_slice(buf.as_slice()).unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            assert_eq!(evt.line_seqno, 0);
            assert_eq!(evt.seqno, 0);
        }

        #[test]
        fn edge_event_size() {
            let sim = gpiosim::simpleton(3);
            let simc = sim.chip();
            let offset = 0;

            let mut builder = Request::builder();
            builder.on_chip(&simc.dev_path).with_line(offset).as_input();
            #[cfg(feature = "uapi_v2")]
            builder.using_abi_version(gpiocdev::AbiVersion::V1);

            let req = builder.request().unwrap();
            let ees = req.edge_event_size();
            assert_eq!(ees, 16);
        }
    }

    #[cfg(feature = "uapi_v2")]
    mod uapi_v2 {
        use super::propagation_delay;
        use gpiocdev::request::Request;
        use gpiocdev::AbiVersion::V2;
        use std::time::Duration;

        #[test]
        fn value() {
            super::value(V2);
        }

        #[test]
        fn values() {
            super::values(V2);
        }

        #[test]
        fn set_value() {
            super::set_value(V2);
        }

        #[test]
        fn set_values() {
            super::set_values(V2);
        }

        #[test]
        fn reconfigure() {
            super::reconfigure(V2);
        }

        #[test]
        fn has_edge_event() {
            super::has_edge_event(V2);
        }

        #[test]
        fn wait_edge_event() {
            super::wait_edge_event(V2);
        }

        #[test]
        fn read_edge_event() {
            super::read_edge_event(V2);
        }

        #[test]
        fn read_edge_events_into_slice() {
            super::read_edge_events_into_slice(V2);
        }

        #[test]
        fn new_edge_event_buffer() {
            super::new_edge_event_buffer(V2);
        }

        #[test]
        fn reconfigure_too_complicated() {
            let sim = gpiosim::simpleton(20);
            let simc = sim.chip();
            let offsets = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

            let req = Request::builder()
                .on_chip(&simc.dev_path)
                .with_lines(offsets)
                .as_input()
                .request()
                .unwrap();

            let mut cfg = req.config();
            for offset in 1..15 {
                cfg.with_line(offset)
                    .with_debounce_period(Duration::from_millis(offset as u64));
            }
            let res = req.reconfigure(&cfg);
            assert_eq!(
                res.unwrap_err().to_string(),
                "uAPI ABI v2 supports 10 attrs, configuration requires 14."
            );
        }

        #[test]
        fn reconfigure_ignores_unrequested_lines() {
            let sim = gpiosim::simpleton(20);
            let simc = sim.chip();
            let offsets = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

            let req = Request::builder()
                .on_chip(&simc.dev_path)
                .with_lines(offsets)
                .as_input()
                .request()
                .unwrap();

            let mut cfg = req.config();
            for offset in 1..15 {
                cfg.with_line(offset)
                    .with_debounce_period(Duration::from_millis(offset as u64));
            }
            assert!(req.reconfigure(&cfg).is_ok());
        }

        #[test]
        fn edge_events() {
            let sim = gpiosim::simpleton(3);
            let simc = sim.chip();
            let offsets = &[1, 2];

            let req = Request::builder()
                .on_chip(&simc.dev_path)
                .with_lines(offsets)
                .as_input()
                .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
                .request()
                .unwrap();

            // create four events
            simc.toggle(1).unwrap();
            propagation_delay();
            simc.toggle(2).unwrap();
            propagation_delay();
            simc.toggle(1).unwrap();
            propagation_delay();
            simc.toggle(2).unwrap();
            propagation_delay();

            let mut iter = req.edge_events().unwrap();
            let evt = iter.next().unwrap().unwrap();
            assert_eq!(evt.offset, 1);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            assert_eq!(evt.line_seqno, 1);
            assert_eq!(evt.seqno, 1);

            let evt = iter.next().unwrap().unwrap();
            assert_eq!(evt.offset, 2);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            assert_eq!(evt.line_seqno, 1);
            assert_eq!(evt.seqno, 2);

            let evt = iter.next().unwrap().unwrap();
            assert_eq!(evt.offset, 1);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            assert_eq!(evt.line_seqno, 2);
            assert_eq!(evt.seqno, 3);

            let evt = iter.next().unwrap().unwrap();
            assert_eq!(evt.offset, 2);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            assert_eq!(evt.line_seqno, 2);
            assert_eq!(evt.seqno, 4);
        }

        #[test]
        #[cfg(feature = "uapi_v2")]
        fn edge_event_from_slice() {
            let sim = gpiosim::simpleton(3);
            let simc = sim.chip();
            let offsets = &[1, 2];

            let req = Request::builder()
                .on_chip(&simc.dev_path)
                .with_lines(offsets)
                .as_input()
                .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
                .request()
                .unwrap();
            let mut buf = vec![0; req.edge_event_size() * 3];

            // create four events
            simc.toggle(1).unwrap();
            propagation_delay();
            simc.toggle(2).unwrap();
            propagation_delay();
            simc.toggle(1).unwrap();
            propagation_delay();
            simc.toggle(2).unwrap();
            propagation_delay();

            // read a buffer full
            let wlen = req.read_edge_events_into_slice(buf.as_mut_slice()).unwrap();
            assert_eq!(wlen, buf.capacity());

            let evt = req.edge_event_from_slice(buf.as_slice()).unwrap();
            assert_eq!(evt.offset, 1);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            assert_eq!(evt.line_seqno, 1);
            assert_eq!(evt.seqno, 1);

            let evt = req
                .edge_event_from_slice(&buf.as_slice()[req.edge_event_size()..])
                .unwrap();
            assert_eq!(evt.offset, 2);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            assert_eq!(evt.line_seqno, 1);
            assert_eq!(evt.seqno, 2);

            let evt = req
                .edge_event_from_slice(&buf.as_slice()[req.edge_event_size() * 2..])
                .unwrap();
            assert_eq!(evt.offset, 1);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            assert_eq!(evt.line_seqno, 2);
            assert_eq!(evt.seqno, 3);

            // read remaining event
            let wlen = req.read_edge_events_into_slice(buf.as_mut_slice()).unwrap();
            assert_eq!(wlen, req.edge_event_size());

            let evt = req.edge_event_from_slice(buf.as_slice()).unwrap();
            assert_eq!(evt.offset, 2);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            assert_eq!(evt.line_seqno, 2);
            assert_eq!(evt.seqno, 4);
        }

        #[test]
        fn edge_event_size() {
            let sim = gpiosim::simpleton(3);
            let simc = sim.chip();
            let offset = 2;

            // v2
            let req = Request::builder()
                .on_chip(&simc.dev_path)
                .with_line(offset)
                .as_input()
                .request()
                .unwrap();

            let ees_v2 = req.edge_event_size();
            assert_eq!(ees_v2, 48);
        }
    }

    #[allow(unused)]
    fn value(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offsets = &[0, 1, 2];

        let mut builder = Request::builder();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = builder
            .on_chip(&simc.dev_path)
            .with_lines(offsets)
            .as_input()
            .request()
            .unwrap();

        for offset in offsets {
            let v = req.value(*offset).unwrap();
            assert_eq!(v, Value::Inactive);

            simc.pullup(*offset).unwrap();
            propagation_delay();
            let v = req.value(*offset).unwrap();
            assert_eq!(v, Value::Active);

            simc.pulldown(*offset).unwrap();
            propagation_delay();
            let v = req.value(*offset).unwrap();
            assert_eq!(v, Value::Inactive);
        }

        // invalid offset
        let res = req.value(3);
        assert_eq!(
            res.unwrap_err(),
            gpiocdev::Error::InvalidArgument("offset is not a requested line.".to_string())
        );
    }

    #[allow(unused)]
    fn values(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(5);
        let simc = sim.chip();
        let offsets = &[0, 1, 3];

        let mut builder = Request::builder();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = builder
            .on_chip(&simc.dev_path)
            .with_lines(offsets)
            .as_input()
            .request()
            .unwrap();

        let mut vals = Values::default();

        // full set
        assert!(req.values(&mut vals).is_ok());
        assert_eq!(vals.get(0), Some(Value::Inactive));
        assert_eq!(vals.get(1), Some(Value::Inactive));
        assert_eq!(vals.get(3), Some(Value::Inactive));

        println!("vals: {:?}", vals);
        simc.pullup(1).unwrap();
        simc.pullup(3).unwrap();
        propagation_delay();
        assert!(req.values(&mut vals).is_ok());
        println!("vals: {:?}", vals);
        assert_eq!(vals.get(0), Some(Value::Inactive));
        assert_eq!(vals.get(1), Some(Value::Active));
        assert_eq!(vals.get(3), Some(Value::Active));

        simc.pullup(0).unwrap();
        simc.pulldown(3).unwrap();
        propagation_delay();
        assert!(req.values(&mut vals).is_ok());
        assert_eq!(vals.get(0), Some(Value::Active));
        assert_eq!(vals.get(1), Some(Value::Active));
        assert_eq!(vals.get(3), Some(Value::Inactive));

        // explicit full set
        let mut vals = Values::from_offsets(offsets);
        simc.pulldown(0).unwrap();
        simc.pullup(3).unwrap();
        propagation_delay();
        assert!(req.values(&mut vals).is_ok());
        assert_eq!(vals.get(0), Some(Value::Inactive));
        assert_eq!(vals.get(1), Some(Value::Active));
        assert_eq!(vals.get(3), Some(Value::Active));

        // subset
        simc.pulldown(0).unwrap();
        propagation_delay();
        let mut vals = Values::from_offsets(&[0, 3]);
        assert!(req.values(&mut vals).is_ok());
        assert_eq!(vals.get(0), Some(Value::Inactive));
        assert_eq!(vals.get(1), None);
        assert_eq!(vals.get(3), Some(Value::Active));

        // singular
        let mut vals = Values::from_offsets(&[3]);
        assert!(req.values(&mut vals).is_ok());
        assert_eq!(vals.get(0), None);
        assert_eq!(vals.get(3), Some(Value::Active));

        // invalid offset - unaltered
        let mut vals = Values::from_offsets(&[1, 2]);
        assert!(req.values(&mut vals).is_ok());
        assert_eq!(vals.get(0), None);
        assert_eq!(vals.get(1), Some(Value::Active));
        assert_eq!(vals.get(2), Some(Value::Inactive));
        assert_eq!(vals.get(3), None);
    }

    #[allow(unused)]
    fn set_value(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offsets = &[0, 1, 2];

        let mut builder = Request::builder();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = builder
            .on_chip(&simc.dev_path)
            .with_lines(offsets)
            .as_output(Value::Inactive)
            .request()
            .unwrap();

        if abiv == gpiocdev::AbiVersion::V2 {
            for offset in offsets {
                assert_eq!(simc.get_level(*offset).unwrap(), gpiosim::Level::Low);

                assert!(req.set_value(*offset, Value::Active).is_ok());
                assert_eq!(simc.get_level(*offset).unwrap(), gpiosim::Level::High);

                assert!(req.set_value(*offset, Value::Inactive).is_ok());
                assert_eq!(simc.get_level(*offset).unwrap(), gpiosim::Level::Low);
            }
        } else {
            for offset in offsets {
                assert_eq!(
                    req.set_value(*offset, Value::Active).unwrap_err(),
                    gpiocdev::Error::AbiLimitation(
                        gpiocdev::AbiVersion::V1,
                        "requires all requested lines".to_string(),
                    )
                );
            }
        }

        // invalid offset
        let res = req.set_value(3, Value::Active);
        assert_eq!(
            res.unwrap_err(),
            gpiocdev::Error::InvalidArgument("offset is not a requested line.".to_string())
        );
    }

    #[allow(unused)]
    fn set_values(abiv: gpiocdev::AbiVersion) {
        use gpiosim::Level;

        let sim = gpiosim::simpleton(5);
        let simc = sim.chip();
        let offsets = &[0, 1, 3];

        let mut builder = Request::builder();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = builder
            .on_chip(&simc.dev_path)
            .with_lines(offsets)
            .as_output(Value::Active)
            .request()
            .unwrap();

        let mut vals = Values::from_offsets(offsets);

        // full set
        assert!(req.set_values(&vals).is_ok());
        assert_eq!(simc.get_level(0).unwrap(), Level::Low);
        assert_eq!(simc.get_level(1).unwrap(), Level::Low);
        assert_eq!(simc.get_level(3).unwrap(), Level::Low);

        vals.set(1, Value::Active);
        vals.set(3, Value::Active);
        assert!(req.set_values(&vals).is_ok());
        assert_eq!(simc.get_level(0).unwrap(), Level::Low);
        assert_eq!(simc.get_level(1).unwrap(), Level::High);
        assert_eq!(simc.get_level(3).unwrap(), Level::High);

        vals.set(0, Value::Active);
        vals.set(3, Value::Inactive);
        assert!(req.set_values(&vals).is_ok());
        assert_eq!(simc.get_level(0).unwrap(), Level::High);
        assert_eq!(simc.get_level(1).unwrap(), Level::High);
        assert_eq!(simc.get_level(3).unwrap(), Level::Low);

        if abiv == gpiocdev::AbiVersion::V2 {
            // subset
            let mut vals = Values::default();
            vals.set(1, Value::Inactive);
            vals.set(3, Value::Active);
            assert!(req.set_values(&vals).is_ok());
            assert_eq!(simc.get_level(0).unwrap(), Level::High);
            assert_eq!(simc.get_level(1).unwrap(), Level::Low);
            assert_eq!(simc.get_level(3).unwrap(), Level::High);

            // singular
            let mut vals = Values::default();
            vals.set(3, Value::Inactive);
            assert!(req.set_values(&vals).is_ok());
            assert_eq!(simc.get_level(0).unwrap(), Level::High);
            assert_eq!(simc.get_level(1).unwrap(), Level::Low);
            assert_eq!(simc.get_level(3).unwrap(), Level::Low);
        } else {
            // subset
            let mut vals = Values::default();
            vals.set(1, Value::Inactive);
            vals.set(3, Value::Active);
            assert_eq!(
                req.set_values(&vals).unwrap_err(),
                gpiocdev::Error::AbiLimitation(
                    gpiocdev::AbiVersion::V1,
                    "requires all requested lines".to_string(),
                )
            );

            // singular
            let mut vals = Values::default();
            vals.set(3, Value::Inactive);
            assert_eq!(
                req.set_values(&vals).unwrap_err(),
                gpiocdev::Error::AbiLimitation(
                    gpiocdev::AbiVersion::V1,
                    "requires all requested lines".to_string(),
                )
            );
        }

        // invalid offset - ignored
        let mut vals = Values::from_offsets(&[0, 1, 2, 3]);
        vals.set(0, Value::Active);
        vals.set(1, Value::Active);
        assert!(req.set_values(&vals).is_ok());
        assert_eq!(simc.get_level(0).unwrap(), Level::High);
        assert_eq!(simc.get_level(1).unwrap(), Level::High);
        assert_eq!(simc.get_level(3).unwrap(), Level::Low);
    }

    #[allow(unused)]
    fn reconfigure(abiv: gpiocdev::AbiVersion) {
        use gpiocdev::line::{Bias, Direction, Drive, EdgeDetection};
        use gpiosim::Level;

        let sim = gpiosim::simpleton(5);
        let simc = sim.chip();
        let cdevc = gpiocdev::chip::Chip::from_path(&simc.dev_path).unwrap();
        let offset = 1;

        let mut builder = Request::builder();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = builder
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_active_low()
            .with_bias(gpiocdev::line::Bias::PullDown)
            .with_drive(gpiocdev::line::Drive::OpenDrain)
            .request()
            .unwrap();

        let info = cdevc.line_info(offset).unwrap();
        assert!(info.active_low);
        assert_eq!(info.direction, Direction::Output);
        assert_eq!(info.bias, Some(Bias::PullDown));
        assert_eq!(info.drive, Some(Drive::OpenDrain));
        assert_eq!(info.edge_detection, None);
        assert_eq!(info.debounce_period, None);
        assert_eq!(simc.get_level(offset).unwrap(), Level::Low);

        let mut cfg = req.config();
        cfg.with_bias(gpiocdev::line::Bias::PullUp)
            .with_drive(gpiocdev::line::Drive::OpenSource)
            .as_active_high();
        assert!(req.reconfigure(&cfg).is_ok());

        let info = cdevc.line_info(offset).unwrap();
        assert!(!info.active_low);
        assert_eq!(info.direction, Direction::Output);
        assert_eq!(info.bias, Some(Bias::PullUp));
        assert_eq!(info.drive, Some(Drive::OpenSource));
        assert_eq!(info.edge_detection, None);
        assert_eq!(info.debounce_period, None);
        assert_eq!(simc.get_level(offset).unwrap(), Level::High);
    }

    #[allow(unused)]
    fn has_edge_event(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 2;

        let mut builder = Request::builder();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = builder
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .request()
            .unwrap();
        assert_eq!(req.has_edge_event(), Ok(false));

        simc.pullup(offset).unwrap();
        propagation_delay();
        assert_eq!(req.has_edge_event(), Ok(true));
    }

    #[allow(unused)]
    fn wait_edge_event(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 2;

        let mut builder = Request::builder();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = builder
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .request()
            .unwrap();

        assert_eq!(req.wait_edge_event(Duration::from_millis(1)), Ok(false));

        simc.pullup(offset).unwrap();
        assert_eq!(req.wait_edge_event(Duration::from_millis(1)), Ok(true));
    }

    #[allow(unused)]
    fn read_edge_event(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 1;

        let mut builder = Request::builder();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = builder
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .request()
            .unwrap();

        simc.pullup(offset).unwrap();
        assert_eq!(req.wait_edge_event(Duration::from_millis(1)), Ok(true));
        let evt = req.read_edge_event().unwrap();
        assert_eq!(evt.kind, EdgeKind::Rising);
        assert_eq!(evt.offset, offset);
        if abiv == gpiocdev::AbiVersion::V2 {
            assert_eq!(evt.line_seqno, 1);
            assert_eq!(evt.seqno, 1);
        } else {
            assert_eq!(evt.line_seqno, 0);
            assert_eq!(evt.seqno, 0);
        }

        simc.pulldown(offset).unwrap();
        assert_eq!(req.wait_edge_event(Duration::from_millis(1)), Ok(true));
        let evt = req.read_edge_event().unwrap();
        assert_eq!(evt.kind, EdgeKind::Falling);
        if abiv == gpiocdev::AbiVersion::V2 {
            assert_eq!(evt.line_seqno, 2);
            assert_eq!(evt.seqno, 2);
        } else {
            assert_eq!(evt.line_seqno, 0);
            assert_eq!(evt.seqno, 0);
        }
    }

    #[allow(unused)]
    fn read_edge_events_into_slice(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 2;

        let mut builder = Request::builder();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);
        let req = builder
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .request()
            .unwrap();
        let mut buf = vec![0; req.edge_event_size() * 3];

        // create four events
        simc.toggle(offset).unwrap();
        propagation_delay();
        simc.toggle(offset).unwrap();
        propagation_delay();
        simc.toggle(offset).unwrap();
        propagation_delay();
        simc.toggle(offset).unwrap();
        propagation_delay();

        // read a buffer full
        let wlen = req.read_edge_events_into_slice(buf.as_mut_slice()).unwrap();
        assert_eq!(wlen, buf.capacity());

        // read remaining event
        let wlen = req.read_edge_events_into_slice(buf.as_mut_slice()).unwrap();
        assert_eq!(wlen, req.edge_event_size());
    }

    #[allow(unused)]
    fn new_edge_event_buffer(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 2;

        let mut builder = Request::builder();
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = builder
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .request()
            .unwrap();
        let mut buf = req.new_edge_event_buffer(4);
        assert_eq!(buf.has_event(), Ok(false));
        assert_eq!(buf.capacity(), 4);
    }
}

mod edge_event_buffer {
    use super::*;

    #[test]
    fn capacity() {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 2;

        let req = Request::builder()
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .request()
            .unwrap();
        let buf = req.new_edge_event_buffer(4);
        assert_eq!(buf.capacity(), 4);
        let buf = req.new_edge_event_buffer(1);
        assert_eq!(buf.capacity(), 1);
        // no capacity makes no sense - force to 1
        let buf = req.new_edge_event_buffer(0);
        assert_eq!(buf.capacity(), 1);
    }

    #[test]
    fn len() {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 2;

        let req = Request::builder()
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .request()
            .unwrap();
        let mut buf = req.new_edge_event_buffer(2);
        assert!(buf.is_empty());

        // create four events
        simc.toggle(offset).unwrap();
        propagation_delay();
        simc.toggle(offset).unwrap();
        propagation_delay();
        simc.toggle(offset).unwrap();
        propagation_delay();
        simc.toggle(offset).unwrap();
        propagation_delay();
        assert_eq!(buf.len(), 0);

        // read one (copy two to buffer and return the first)
        _ = buf.read_event().unwrap();
        // should still be one buffered
        assert_eq!(buf.len(), 1);

        // read last one buffered
        _ = buf.read_event().unwrap();
        assert!(buf.is_empty());

        // read one (copy remaining two to buffer from kernel and return the first)
        _ = buf.read_event().unwrap();
        // should still be one buffered
        assert_eq!(buf.len(), 1);

        // read last one buffered
        _ = buf.read_event().unwrap();
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn is_empty() {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 2;

        let req = Request::builder()
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .request()
            .unwrap();
        let mut buf = req.new_edge_event_buffer(4);
        assert!(buf.is_empty());

        // create two events
        simc.toggle(offset).unwrap();
        propagation_delay();
        simc.toggle(offset).unwrap();
        propagation_delay();
        assert!(buf.is_empty());

        // read one
        _ = buf.read_event().unwrap();
        // should still be one buffered
        assert!(!buf.is_empty());

        // read last one
        _ = buf.read_event().unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn has_event() {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 2;

        let req = Request::builder()
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .request()
            .unwrap();
        let mut buf = req.new_edge_event_buffer(4);
        assert_eq!(buf.has_event(), Ok(false));

        simc.pullup(offset).unwrap();
        propagation_delay();
        assert_eq!(buf.has_event(), Ok(true));
        _ = buf.read_event().unwrap();
        assert_eq!(buf.has_event(), Ok(false));
    }

    #[test]
    fn read_event() {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 2;

        let req = Request::builder()
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .request()
            .unwrap();
        let mut buf = req.new_edge_event_buffer(4);
        assert_eq!(buf.has_event(), Ok(false));

        simc.pullup(offset).unwrap();
        propagation_delay();
        assert_eq!(buf.has_event(), Ok(true));
        let evt = buf.read_event().unwrap();
        assert_eq!(evt.kind, EdgeKind::Rising);
        assert_eq!(evt.offset, offset);
        #[cfg(feature = "uapi_v2")]
        {
            assert_eq!(evt.line_seqno, 1);
            assert_eq!(evt.seqno, 1);
        }

        simc.pulldown(offset).unwrap();
        propagation_delay();
        assert_eq!(buf.has_event(), Ok(true));
        let evt = buf.read_event().unwrap();
        assert_eq!(evt.kind, EdgeKind::Falling);
        #[cfg(feature = "uapi_v2")]
        {
            assert_eq!(evt.line_seqno, 2);
            assert_eq!(evt.seqno, 2);
        }
    }

    #[test]
    fn wait_event() {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 2;

        let req = Request::builder()
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .request()
            .unwrap();
        let mut buf = req.new_edge_event_buffer(4);
        assert_eq!(buf.has_event(), Ok(false));

        simc.pullup(offset).unwrap();
        let evt = buf.wait_event(Duration::from_millis(1)).unwrap();
        assert_eq!(evt.kind, EdgeKind::Rising);
        assert_eq!(evt.offset, offset);
        #[cfg(feature = "uapi_v2")]
        {
            assert_eq!(evt.line_seqno, 1);
            assert_eq!(evt.seqno, 1);
        }

        simc.pulldown(offset).unwrap();
        let evt = buf.wait_event(Duration::from_millis(1)).unwrap();
        assert_eq!(evt.kind, EdgeKind::Falling);
        #[cfg(feature = "uapi_v2")]
        {
            assert_eq!(evt.line_seqno, 2);
            assert_eq!(evt.seqno, 2);
        }
    }
}

// allow time for a gpiosim set to propagate to cdev
fn propagation_delay() {
    sleep(Duration::from_millis(1));
}

struct Symlink {
    src: PathBuf,
}

impl Symlink {
    fn new<P: AsRef<Path>>(dst: P, src: P) -> Result<Symlink, std::io::Error> {
        std::os::unix::fs::symlink(&dst, &src)?;
        Ok(Symlink {
            src: src.as_ref().to_path_buf(),
        })
    }
}
impl Drop for Symlink {
    fn drop(&mut self) {
        _ = std::fs::remove_file(&self.src);
    }
}
