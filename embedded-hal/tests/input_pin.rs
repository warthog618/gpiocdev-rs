// SPDX-FileCopyrightText: 2024 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use embedded_hal::digital::InputPin as _;
use gpiocdev::{line::Direction, Request};
use gpiocdev_embedded_hal::InputPin;
use gpiosim::Simpleton;
use std::time::Duration;

// max time to allow events to propagate from the sim to cdev
const PROPAGATION_DELAY: Duration = Duration::from_millis(10);

fn wait_propagation_delay() {
    std::thread::sleep(PROPAGATION_DELAY);
}

#[test]
fn is_high() {
    let s = Simpleton::new(5);

    let offset = 3;
    let mut pin = InputPin::new(s.dev_path(), offset).unwrap();

    assert!(!pin.is_high().unwrap());
    s.pullup(offset).unwrap();
    wait_propagation_delay();
    assert!(pin.is_high().unwrap());
}

#[test]
fn is_low() {
    let s = Simpleton::new(5);

    let offset = 2;
    let mut pin = InputPin::new(s.dev_path(), offset).unwrap();

    assert!(pin.is_low().unwrap());
    s.pullup(offset).unwrap();
    wait_propagation_delay();
    assert!(!pin.is_low().unwrap());
}

#[test]
fn into_output_pin() {
    use embedded_hal::digital::{OutputPin as _, PinState};
    use gpiosim::Level;

    let s = Simpleton::new(5);

    let offset = 2;
    let pin = InputPin::new(s.dev_path(), offset).unwrap();

    // convert to output
    let mut pin = pin.into_output_pin(PinState::Low).unwrap();

    assert_eq!(s.get_level(offset).unwrap(), Level::Low);
    pin.set_high().unwrap();
    assert_eq!(s.get_level(offset).unwrap(), Level::High);

    // check config
    let req = Request::from(pin);
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config.line_config(offset).unwrap();
    assert_eq!(line_config.direction, Some(Direction::Output));
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

        let req = Request::from_config(config).request().unwrap();

        assert_eq!(
            InputPin::try_from(req).unwrap_err(),
            gpiocdev_embedded_hal::Error::RequiresInputMode
        );
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

        let config = req.config();

        // convert to OutputPin
        let mut pin = InputPin::try_from(req).unwrap();

        assert!(pin.is_low().unwrap());
        s.pullup(offset).unwrap();
        wait_propagation_delay();
        assert!(!pin.is_low().unwrap());

        // check config
        let req = Request::from(pin);
        assert_eq!(config, req.config());
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
            InputPin::try_from(req).unwrap_err(),
            gpiocdev_embedded_hal::Error::RequiresInputMode
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
            InputPin::try_from(req).unwrap_err(),
            gpiocdev_embedded_hal::Error::MultipleLinesRequested
        );
    }
}

#[test]
fn into_request() {
    let s = Simpleton::new(5);

    let offset = 2;
    let pin = InputPin::new(s.dev_path(), offset).unwrap();

    let req = Request::from(pin);
    // check config
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config.line_config(offset).unwrap();
    assert_eq!(line_config.direction, Some(Direction::Input));
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
    let pin = InputPin::from_found_line(fl).unwrap();

    let req = Request::from(pin);
    // check config
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config.line_config(offset).unwrap();
    assert_eq!(line_config.direction, Some(Direction::Input));
}

#[test]
fn from_name() {
    // potential race with other tests if there is a line name collision.
    use gpiosim::Bank;
    let s = gpiosim::builder()
        .with_bank(
            Bank::new(8, "input from_name")
                .name(3, "ifn banana")
                .name(6, "ifn apple"),
        )
        .live()
        .unwrap();

    let offset = 6;
    let pin = InputPin::from_name("ifn apple").unwrap();

    let req = Request::from(pin);
    // check config
    assert_eq!(&req.chip_path(), s.chips()[0].dev_path());
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config.line_config(offset).unwrap();
    assert_eq!(line_config.direction, Some(Direction::Input));

    assert_eq!(
        InputPin::from_name("ifn grape").unwrap_err(),
        gpiocdev_embedded_hal::Error::UnfoundLine("ifn grape".into())
    );
}

#[cfg(feature = "async_tokio")]
mod async_tokio {
    use super::*;
    use embedded_hal_async::digital::Wait;
    use gpiocdev_embedded_hal::tokio::InputPin;
    use gpiocdev_embedded_hal::InputPin as SyncInputPin;

    async fn will_resolve<F>(future: F) -> bool
    where
        F: core::future::Future,
    {
        tokio::time::timeout(PROPAGATION_DELAY, future)
            .await
            .is_ok()
    }

    #[tokio::test]
    async fn from_input() {
        let s = Simpleton::new(5);

        let offset = 3;
        let pin = SyncInputPin::new(s.dev_path(), offset).unwrap();
        let mut pin = InputPin::from(pin);

        assert!(pin.is_low().unwrap());
        s.pullup(offset).unwrap();
        wait_propagation_delay();
        assert!(pin.is_high().unwrap());

        let req = gpiocdev::Request::from(pin);
        let config = req.config();
        let offsets = config.lines();
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0], offset);
        let line_config = config.line_config(offset).unwrap();
        assert_eq!(line_config.direction, Some(Direction::Input));
        assert_eq!(line_config.edge_detection, None);
    }

    #[tokio::test]
    async fn into_input() {
        let s = Simpleton::new(5);

        let offset = 3;
        let pin = InputPin::new(s.dev_path(), offset).unwrap();
        let mut pin = SyncInputPin::from(pin);

        assert!(pin.is_low().unwrap());
        s.pullup(offset).unwrap();
        wait_propagation_delay();
        assert!(pin.is_high().unwrap());

        let req = gpiocdev::Request::from(pin);
        let config = req.config();
        let offsets = config.lines();
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0], offset);
        let line_config = config.line_config(offset).unwrap();
        assert_eq!(line_config.direction, Some(Direction::Input));
        assert_eq!(line_config.edge_detection, None);
    }

    #[tokio::test]
    async fn wait_for_high() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).unwrap();

        // known low
        assert!(!will_resolve(pin.wait_for_high()).await);
        assert!(will_resolve(pin.wait_for_low()).await);
        // switched to high
        s.pullup(offset).unwrap();
        assert!(will_resolve(pin.wait_for_high()).await);
        assert!(!will_resolve(pin.wait_for_low()).await);
        assert!(will_resolve(pin.wait_for_high()).await);
        // switched to low
        s.pulldown(offset).unwrap();
        assert!(will_resolve(pin.wait_for_low()).await);
        assert!(!will_resolve(pin.wait_for_high()).await);
    }

    #[tokio::test]
    async fn wait_for_low() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).unwrap();

        // known low
        assert!(will_resolve(pin.wait_for_low()).await);
        // switched to high
        s.pullup(offset).unwrap();
        wait_propagation_delay();
        // Possible quirk - impl doesn't check for events if the last known level
        // is what you asked for - it does not check the hardware and so doesn't see
        // high until you explicitly check for high...
        // Safe to assume anyone using level logic will always be flip-flopping?
        assert!(will_resolve(pin.wait_for_low()).await);
        assert!(will_resolve(pin.wait_for_high()).await);
        assert!(!will_resolve(pin.wait_for_low()).await);
        // switched to low
        s.pulldown(offset).unwrap();
        assert!(will_resolve(pin.wait_for_low()).await);
        assert!(!will_resolve(pin.wait_for_high()).await);
    }

    #[tokio::test]
    async fn wait_for_rising_edge() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).unwrap();

        // known low
        assert!(!will_resolve(pin.wait_for_rising_edge()).await);
        // switched to high
        s.pullup(offset).unwrap();
        assert!(will_resolve(pin.wait_for_rising_edge()).await);
        assert!(!will_resolve(pin.wait_for_rising_edge()).await);
        // switched to low
        s.pulldown(offset).unwrap();
        assert!(!will_resolve(pin.wait_for_rising_edge()).await);
        // switched to high
        s.pullup(offset).unwrap();
        assert!(will_resolve(pin.wait_for_rising_edge()).await);
        assert!(!will_resolve(pin.wait_for_rising_edge()).await);
    }

    #[tokio::test]
    async fn wait_for_falling_edge() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).unwrap();

        // known low
        assert!(!will_resolve(pin.wait_for_falling_edge()).await);
        // switched to high
        s.pullup(offset).unwrap();
        wait_propagation_delay();
        assert!(!will_resolve(pin.wait_for_falling_edge()).await);
        // switched to low
        s.pulldown(offset).unwrap();
        assert!(will_resolve(pin.wait_for_falling_edge()).await);
        assert!(!will_resolve(pin.wait_for_falling_edge()).await);
        // switched to high
        s.pullup(offset).unwrap();
        assert!(!will_resolve(pin.wait_for_falling_edge()).await);
    }

    #[tokio::test]
    async fn wait_for_any_edge() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).unwrap();

        // known low
        assert!(!will_resolve(pin.wait_for_any_edge()).await);
        // switched to high
        s.pullup(offset).unwrap();
        assert!(will_resolve(pin.wait_for_any_edge()).await);
        // switched to low
        s.pulldown(offset).unwrap();
        assert!(will_resolve(pin.wait_for_any_edge()).await);
        assert!(!will_resolve(pin.wait_for_any_edge()).await);
        // switched to high
        s.pullup(offset).unwrap();
        assert!(will_resolve(pin.wait_for_any_edge()).await);
        assert!(!will_resolve(pin.wait_for_any_edge()).await);
    }
}

#[cfg(feature = "async_io")]
mod async_io {
    use super::*;
    use ::async_io::block_on;
    use embedded_hal_async::digital::Wait;
    use gpiocdev_embedded_hal::async_io::InputPin;
    use gpiocdev_embedded_hal::InputPin as SyncInputPin;

    async fn will_resolve<F>(future: F) -> bool
    where
        F: core::future::Future,
    {
        async_std::future::timeout(PROPAGATION_DELAY, future)
            .await
            .is_ok()
    }

    #[test]
    fn from_input() {
        let s = Simpleton::new(5);

        let offset = 3;
        let pin = SyncInputPin::new(s.dev_path(), offset).unwrap();
        let mut pin = InputPin::from(pin);

        assert!(pin.is_low().unwrap());
        s.pullup(offset).unwrap();
        wait_propagation_delay();
        assert!(pin.is_high().unwrap());

        let req = gpiocdev::Request::from(pin);
        let config = req.config();
        let offsets = config.lines();
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0], offset);
        let line_config = config.line_config(offset).unwrap();
        assert_eq!(line_config.direction, Some(Direction::Input));
        assert_eq!(line_config.edge_detection, None);
    }

    #[test]
    fn into_input() {
        let s = Simpleton::new(5);

        let offset = 3;
        let pin = InputPin::new(s.dev_path(), offset).unwrap();
        let mut pin = SyncInputPin::from(pin);

        assert!(pin.is_low().unwrap());
        s.pullup(offset).unwrap();
        wait_propagation_delay();
        assert!(pin.is_high().unwrap());

        let req = gpiocdev::Request::from(pin);
        let config = req.config();
        let offsets = config.lines();
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0], offset);
        let line_config = config.line_config(offset).unwrap();
        assert_eq!(line_config.direction, Some(Direction::Input));
        assert_eq!(line_config.edge_detection, None);
    }

    #[test]
    fn wait_for_high() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).unwrap();

        block_on(async {
            // known low
            assert!(!will_resolve(pin.wait_for_high()).await);
            assert!(will_resolve(pin.wait_for_low()).await);
            // switched to high
            s.pullup(offset).unwrap();
            assert!(will_resolve(pin.wait_for_high()).await);
            assert!(!will_resolve(pin.wait_for_low()).await);
            assert!(will_resolve(pin.wait_for_high()).await);
            // switched to low
            s.pulldown(offset).unwrap();
            assert!(will_resolve(pin.wait_for_low()).await);
            assert!(!will_resolve(pin.wait_for_high()).await);
        })
    }

    #[test]
    fn wait_for_low() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).unwrap();

        block_on(async {
            // known low
            assert!(will_resolve(pin.wait_for_low()).await);
            // switched to high
            s.pullup(offset).unwrap();
            wait_propagation_delay();
            // Possible quirk - impl doesn't check for events if the last known level
            // is what you asked for - it does not check the hardware and so doesn't see
            // high until you explicitly check for high...
            // Safe to assume anyone using level logic will always be flip-flopping?
            assert!(will_resolve(pin.wait_for_low()).await);
            assert!(will_resolve(pin.wait_for_high()).await);
            assert!(!will_resolve(pin.wait_for_low()).await);
            // switched to low
            s.pulldown(offset).unwrap();
            assert!(will_resolve(pin.wait_for_low()).await);
            assert!(!will_resolve(pin.wait_for_high()).await);
        })
    }

    #[test]
    fn wait_for_rising_edge() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).unwrap();

        block_on(async {
            // known low
            assert!(!will_resolve(pin.wait_for_rising_edge()).await);
            // switched to high
            s.pullup(offset).unwrap();
            assert!(will_resolve(pin.wait_for_rising_edge()).await);
            assert!(!will_resolve(pin.wait_for_rising_edge()).await);
            // switched to low
            s.pulldown(offset).unwrap();
            assert!(!will_resolve(pin.wait_for_rising_edge()).await);
            // switched to high
            s.pullup(offset).unwrap();
            assert!(will_resolve(pin.wait_for_rising_edge()).await);
            assert!(!will_resolve(pin.wait_for_rising_edge()).await);
        })
    }

    #[test]
    fn wait_for_falling_edge() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).unwrap();

        block_on(async {
            // known low
            assert!(!will_resolve(pin.wait_for_falling_edge()).await);
            // switched to high
            s.pullup(offset).unwrap();
            wait_propagation_delay();
            assert!(!will_resolve(pin.wait_for_falling_edge()).await);
            // switched to low
            s.pulldown(offset).unwrap();
            assert!(will_resolve(pin.wait_for_falling_edge()).await);
            assert!(!will_resolve(pin.wait_for_falling_edge()).await);
            // switched to high
            s.pullup(offset).unwrap();
            assert!(!will_resolve(pin.wait_for_falling_edge()).await);
        })
    }

    #[test]
    fn wait_for_any_edge() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).unwrap();

        block_on(async {
            // known low
            assert!(!will_resolve(pin.wait_for_any_edge()).await);
            // switched to high
            s.pullup(offset).unwrap();
            assert!(will_resolve(pin.wait_for_any_edge()).await);
            // switched to low
            s.pulldown(offset).unwrap();
            assert!(will_resolve(pin.wait_for_any_edge()).await);
            assert!(!will_resolve(pin.wait_for_any_edge()).await);
            // switched to high
            s.pullup(offset).unwrap();
            assert!(will_resolve(pin.wait_for_any_edge()).await);
            assert!(!will_resolve(pin.wait_for_any_edge()).await);
        });
    }
}
