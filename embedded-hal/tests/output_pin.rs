// SPDX-FileCopyrightText: 2024 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use embedded_hal::digital::{OutputPin as _, PinState, StatefulOutputPin};
use gpiocdev::{line::Direction, Request};
use gpiocdev_embedded_hal::OutputPin;
use gpiosim::{Level, Simpleton};

#[test]
fn set_high() {
    let s = Simpleton::new(5);

    let offset = 3;
    let mut pin = OutputPin::new(s.dev_path(), offset, PinState::Low).expect("pin should exist");

    assert_eq!(
        s.get_level(offset).expect("get_level should succeed"),
        Level::Low
    );
    pin.set_high().expect("set_high should succeed");
    assert_eq!(
        s.get_level(offset).expect("get_level should succeed"),
        Level::High
    );
}

#[test]
fn set_low() {
    let s = Simpleton::new(5);

    let offset = 2;
    let mut pin = OutputPin::new(s.dev_path(), offset, PinState::High).expect("pin should exist");

    assert_eq!(
        s.get_level(offset).expect("get_level should succeed"),
        Level::High
    );
    pin.set_low().expect("set_low should succeed");
    assert_eq!(
        s.get_level(offset).expect("get_level should succeed"),
        Level::Low
    );
}

#[test]
fn is_set_high() {
    let s = Simpleton::new(5);

    let offset = 3;
    let mut pin = OutputPin::new(s.dev_path(), offset, PinState::Low).expect("pin should exist");

    assert!(!pin.is_set_high().expect("is_set_high should succeed"));
    pin.set_high().expect("set_high should succeed");
    assert!(pin.is_set_high().expect("is_set_high should succeed"));
    pin.set_low().expect("set_low should succeed");
    assert!(!pin.is_set_high().expect("is_set_high should succeed"));
}

#[test]
fn is_set_low() {
    let s = Simpleton::new(5);

    let offset = 3;
    let mut pin = OutputPin::new(s.dev_path(), offset, PinState::Low).expect("pin should exist");

    assert!(pin.is_set_low().expect("is_set_low should succeed"));
    pin.set_high().expect("set_high should succeed");
    assert!(!pin.is_set_low().expect("is_set_low should succeed"));
    pin.set_low().expect("set_low should succeed");
    assert!(pin.is_set_low().expect("is_set_low should succeed"));
}

#[test]
fn toggle() {
    let s = Simpleton::new(5);

    let offset = 2;
    let mut pin = OutputPin::new(s.dev_path(), offset, PinState::High).expect("pin should exist");

    assert_eq!(
        s.get_level(offset).expect("get_level should succeed"),
        Level::High
    );
    pin.toggle().expect("toggle should succeed");
    assert_eq!(
        s.get_level(offset).expect("get_level should succeed"),
        Level::Low
    );
    pin.toggle().expect("toggle should succeed");
    assert_eq!(
        s.get_level(offset).expect("get_level should succeed"),
        Level::High
    );
}

#[test]
fn into_input_pin() {
    use embedded_hal::digital::InputPin as _;

    let s = Simpleton::new(5);

    let offset = 2;
    let pin = OutputPin::new(s.dev_path(), offset, PinState::High).expect("pin should exist");

    assert_eq!(
        s.get_level(offset).expect("get_level should succeed"),
        Level::High
    );

    // convert to input
    let mut pin = pin.into_input_pin().expect("into_input_pin should succeed");

    // line should be pulled high by sim
    assert!(pin.is_high().expect("is_high should succeed"));
    assert_eq!(
        s.get_level(offset).expect("get_level should succeed"),
        Level::High
    );

    s.pulldown(offset).expect("pulldown should succeed");
    assert!(pin.is_low().expect("is_low should succeed"));

    // check config
    let req = Request::from(pin);
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config
        .line_config(offset)
        .expect("line_config should succeed");
    assert_eq!(line_config.direction, Some(Direction::Input));
}

mod try_from_request {
    use super::*;
    use gpiocdev::line::Value;

    #[test]
    fn output_request() {
        let s = Simpleton::new(5);

        let offset = 2;
        let mut config = gpiocdev::request::Config::default();
        config
            .on_chip(s.dev_path())
            .with_line(offset)
            .as_output(Value::Active);

        // create request
        let req = Request::from_config(config)
            .request()
            .expect("request should succeed");
        let config = req.config();

        // convert to OutputPin
        let mut pin = OutputPin::try_from(req).expect("try_from should succeed");

        // check setting follows
        pin.set_low().expect("set_low should succeed");
        assert_eq!(
            s.get_level(offset).expect("get_level should succeed"),
            Level::Low
        );

        // check config
        let req = Request::from(pin);
        assert_eq!(config, req.config());
    }

    #[test]
    fn input_request() {
        let s = Simpleton::new(5);

        let offset = 2;
        let req = Request::builder()
            .on_chip(s.dev_path())
            .with_line(offset)
            .as_input()
            .request()
            .expect("request should succeed");

        assert_eq!(
            OutputPin::try_from(req).expect_err("try_from should fail"),
            gpiocdev_embedded_hal::Error::RequiresOutputMode
        );
    }

    #[test]
    fn as_is_request() {
        let s = Simpleton::new(5);

        let offset = 2;
        let req = Request::builder()
            .on_chip(s.dev_path())
            .with_line(offset)
            .request()
            .expect("request should succeed");

        assert_eq!(
            OutputPin::try_from(req).expect_err("try_from should fail"),
            gpiocdev_embedded_hal::Error::RequiresOutputMode
        );
    }

    #[test]
    fn multiple_lines() {
        let s = Simpleton::new(5);

        let req = Request::builder()
            .on_chip(s.dev_path())
            .with_lines(&[1, 2])
            .as_output(Value::Inactive)
            .request()
            .expect("request should succeed");

        assert_eq!(
            OutputPin::try_from(req).expect_err("try_from should fail"),
            gpiocdev_embedded_hal::Error::MultipleLinesRequested
        );
    }
}

#[test]
fn into_request() {
    let s = Simpleton::new(5);

    let offset = 2;
    let pin = OutputPin::new(s.dev_path(), offset, PinState::High).expect("pin should exist");

    let req = Request::from(pin);
    // check config
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config
        .line_config(offset)
        .expect("line_config should succeed");
    assert_eq!(line_config.direction, Some(Direction::Output));
}

#[test]
fn from_found_line() {
    let s = Simpleton::new(5);

    let offset = 2;
    let fl = gpiocdev::FoundLine {
        chip: s.dev_path().clone(),
        info: gpiocdev::line::Info {
            offset,
            ..Default::default()
        },
    };
    let pin =
        OutputPin::from_found_line(fl, PinState::High).expect("from_found_line should succeed");

    let req = Request::from(pin);
    // check config
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config
        .line_config(offset)
        .expect("line_config should succeed");
    assert_eq!(line_config.direction, Some(Direction::Output));
}

#[test]
fn from_name() {
    // potential race with other tests if there is a line name collision.
    use gpiosim::Bank;
    let s = gpiosim::builder()
        .with_bank(
            Bank::new(8, "output from_name")
                .name(3, "ofn banana")
                .name(6, "ofn apple"),
        )
        .live()
        .expect("gpiosim should go live");

    let offset = 6;
    let pin = OutputPin::from_name("ofn apple", PinState::High).expect("pin should exist");

    let req = Request::from(pin);
    // check config
    assert_eq!(&req.chip_path(), s.chips()[0].dev_path());
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config
        .line_config(offset)
        .expect("line_config should succeed");
    assert_eq!(line_config.direction, Some(Direction::Output));

    assert_eq!(
        OutputPin::from_name("ofn grape", PinState::High).expect_err("from_name should fail"),
        gpiocdev_embedded_hal::Error::UnfoundLine("ofn grape".into())
    );
}
