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
    let mut pin = OutputPin::new(s.dev_path(), offset, PinState::Low).unwrap();

    assert_eq!(s.get_level(offset).unwrap(), Level::Low);
    pin.set_high().unwrap();
    assert_eq!(s.get_level(offset).unwrap(), Level::High);
}

#[test]
fn set_low() {
    let s = Simpleton::new(5);

    let offset = 2;
    let mut pin = OutputPin::new(s.dev_path(), offset, PinState::High).unwrap();

    assert_eq!(s.get_level(offset).unwrap(), Level::High);
    pin.set_low().unwrap();
    assert_eq!(s.get_level(offset).unwrap(), Level::Low);
}

#[test]
fn is_set_high() {
    let s = Simpleton::new(5);

    let offset = 3;
    let mut pin = OutputPin::new(s.dev_path(), offset, PinState::Low).unwrap();

    assert!(!pin.is_set_high().unwrap());
    pin.set_high().unwrap();
    assert!(pin.is_set_high().unwrap());
    pin.set_low().unwrap();
    assert!(!pin.is_set_high().unwrap());
}

#[test]
fn is_set_low() {
    let s = Simpleton::new(5);

    let offset = 3;
    let mut pin = OutputPin::new(s.dev_path(), offset, PinState::Low).unwrap();

    assert!(pin.is_set_low().unwrap());
    pin.set_high().unwrap();
    assert!(!pin.is_set_low().unwrap());
    pin.set_low().unwrap();
    assert!(pin.is_set_low().unwrap());
}

#[test]
fn toggle() {
    let s = Simpleton::new(5);

    let offset = 2;
    let mut pin = OutputPin::new(s.dev_path(), offset, PinState::High).unwrap();

    assert_eq!(s.get_level(offset).unwrap(), Level::High);
    pin.toggle().unwrap();
    assert_eq!(s.get_level(offset).unwrap(), Level::Low);
    pin.toggle().unwrap();
    assert_eq!(s.get_level(offset).unwrap(), Level::High);
}

#[test]
fn into_input_pin() {
    use embedded_hal::digital::InputPin as _;

    let s = Simpleton::new(5);

    let offset = 2;
    let pin = OutputPin::new(s.dev_path(), offset, PinState::High).unwrap();

    assert_eq!(s.get_level(offset).unwrap(), Level::High);

    // convert to input
    let mut pin = pin.into_input_pin().unwrap();

    // line should be pulled high by sim
    assert!(pin.is_high().unwrap());
    assert_eq!(s.get_level(offset).unwrap(), Level::High);

    s.pulldown(offset).unwrap();
    assert!(pin.is_low().unwrap());

    // check config
    let req = Request::from(pin);
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config.line_config(offset).unwrap();
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
        let req = Request::from_config(config).request().unwrap();
        let config = req.config();

        // convert to OutputPin
        let mut pin = OutputPin::try_from(req).unwrap();

        // check setting follows
        pin.set_low().unwrap();
        assert_eq!(s.get_level(offset).unwrap(), Level::Low);

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
            .unwrap();

        assert_eq!(
            OutputPin::try_from(req).unwrap_err(),
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
            .unwrap();

        assert_eq!(
            OutputPin::try_from(req).unwrap_err(),
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
            .unwrap();

        assert_eq!(
            OutputPin::try_from(req).unwrap_err(),
            gpiocdev_embedded_hal::Error::MultipleLinesRequested
        );
    }
}

#[test]
fn into_request() {
    let s = Simpleton::new(5);

    let offset = 2;
    let pin = OutputPin::new(s.dev_path(), offset, PinState::High).unwrap();

    let req = Request::from(pin);
    // check config
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config.line_config(offset).unwrap();
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
    let pin = OutputPin::from_found_line(fl, PinState::High).unwrap();

    let req = Request::from(pin);
    // check config
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config.line_config(offset).unwrap();
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
        .unwrap();

    let offset = 6;
    let pin = OutputPin::from_name("ofn apple", PinState::High).unwrap();

    let req = Request::from(pin);
    // check config
    assert_eq!(&req.chip_path(), s.chips()[0].dev_path());
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config.line_config(offset).unwrap();
    assert_eq!(line_config.direction, Some(Direction::Output));

    assert_eq!(
        OutputPin::from_name("ofn grape", PinState::High).unwrap_err(),
        gpiocdev_embedded_hal::Error::UnfoundLine("ofn grape".into())
    );
}
