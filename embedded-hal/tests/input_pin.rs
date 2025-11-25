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
    let mut pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

    assert!(!pin.is_high().expect("is_high should succeed"));
    s.pullup(offset).expect("pullup should succeed");
    wait_propagation_delay();
    assert!(pin.is_high().expect("is_high should succeed"));
}

#[test]
fn is_low() {
    let s = Simpleton::new(5);

    let offset = 2;
    let mut pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

    assert!(pin.is_low().expect("is_low should succeed"));
    s.pullup(offset).expect("pullup should succeed");
    wait_propagation_delay();
    assert!(!pin.is_low().expect("is_low should succeed"));
}

#[test]
fn into_output_pin() {
    use embedded_hal::digital::{OutputPin as _, PinState};
    use gpiosim::Level;

    let s = Simpleton::new(5);

    let offset = 2;
    let pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

    // convert to output
    let mut pin = pin
        .into_output_pin(PinState::Low)
        .expect("into_output_pin should succeed");

    assert_eq!(
        s.get_level(offset).expect("get_level should succeed"),
        Level::Low
    );
    pin.set_high().expect("set_high should succeed");
    assert_eq!(
        s.get_level(offset).expect("get_level should succeed"),
        Level::High
    );

    // check config
    let req = Request::from(pin);
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config
        .line_config(offset)
        .expect("line_config should succeed");
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

        let req = Request::from_config(config)
            .request()
            .expect("request should succeed");

        assert_eq!(
            InputPin::try_from(req).expect_err("try_from should fail"),
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
            .expect("request should succeed");

        let config = req.config();

        // convert to OutputPin
        let mut pin = InputPin::try_from(req).expect("try_from should succeed");

        assert!(pin.is_low().expect("is_low should succeed"));
        s.pullup(offset).expect("pullup should succeed");
        wait_propagation_delay();
        assert!(!pin.is_low().expect("is_low should succeed"));

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
            .expect("request should succeed");

        assert_eq!(
            InputPin::try_from(req).expect_err("try_from should fail"),
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
            .expect("request should succeed");

        assert_eq!(
            InputPin::try_from(req).expect_err("try_from should fail"),
            gpiocdev_embedded_hal::Error::MultipleLinesRequested
        );
    }
}

#[test]
fn into_request() {
    let s = Simpleton::new(5);

    let offset = 2;
    let pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

    let req = Request::from(pin);
    // check config
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config
        .line_config(offset)
        .expect("line_config should succeed");
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
    let pin = InputPin::from_found_line(fl).expect("from_found_line should succeed");

    let req = Request::from(pin);
    // check config
    let config = req.config();
    let offsets = config.lines();
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0], offset);
    let line_config = config
        .line_config(offset)
        .expect("line_config should succeed");
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
        .expect("gpiosim should go live");

    let offset = 6;
    let pin = InputPin::from_name("ifn apple").expect("from_name should succeed");

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
    assert_eq!(line_config.direction, Some(Direction::Input));

    assert_eq!(
        InputPin::from_name("ifn grape").expect_err("from_name should fail"),
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
        let pin = SyncInputPin::new(s.dev_path(), offset).expect("pin should exist");
        let mut pin = InputPin::from(pin);

        assert!(pin.is_low().expect("is_low should succeed"));
        s.pullup(offset).expect("pullup should succeed");
        wait_propagation_delay();
        assert!(pin.is_high().expect("is_high should succeed"));

        let req = gpiocdev::Request::from(pin);
        let config = req.config();
        let offsets = config.lines();
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0], offset);
        let line_config = config
            .line_config(offset)
            .expect("line_config should succeed");
        assert_eq!(line_config.direction, Some(Direction::Input));
        assert_eq!(line_config.edge_detection, None);
    }

    #[tokio::test]
    async fn into_input() {
        let s = Simpleton::new(5);

        let offset = 3;
        let pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");
        let mut pin = SyncInputPin::from(pin);

        assert!(pin.is_low().expect("is_low should succeed"));
        s.pullup(offset).expect("pullup should succeed");
        wait_propagation_delay();
        assert!(pin.is_high().expect("is_high should succeed"));

        let req = gpiocdev::Request::from(pin);
        let config = req.config();
        let offsets = config.lines();
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0], offset);
        let line_config = config
            .line_config(offset)
            .expect("line_config should succeed");
        assert_eq!(line_config.direction, Some(Direction::Input));
        assert_eq!(line_config.edge_detection, None);
    }

    #[tokio::test]
    async fn wait_for_high() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

        // known low
        assert!(!will_resolve(pin.wait_for_high()).await);
        assert!(will_resolve(pin.wait_for_low()).await);
        // switched to high
        s.pullup(offset).expect("pullup should succeed");
        assert!(will_resolve(pin.wait_for_high()).await);
        assert!(!will_resolve(pin.wait_for_low()).await);
        assert!(will_resolve(pin.wait_for_high()).await);
        // switched to low
        s.pulldown(offset).expect("pulldown should succeed");
        assert!(will_resolve(pin.wait_for_low()).await);
        assert!(!will_resolve(pin.wait_for_high()).await);
    }

    #[tokio::test]
    async fn wait_for_low() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

        // known low
        assert!(will_resolve(pin.wait_for_low()).await);
        // switched to high
        s.pullup(offset).expect("pullup should succeed");
        wait_propagation_delay();
        // Possible quirk - impl doesn't check for events if the last known level
        // is what you asked for - it does not check the hardware and so doesn't see
        // high until you explicitly check for high...
        // Safe to assume anyone using level logic will always be flip-flopping?
        assert!(will_resolve(pin.wait_for_low()).await);
        assert!(will_resolve(pin.wait_for_high()).await);
        assert!(!will_resolve(pin.wait_for_low()).await);
        // switched to low
        s.pulldown(offset).expect("pulldown should succeed");
        assert!(will_resolve(pin.wait_for_low()).await);
        assert!(!will_resolve(pin.wait_for_high()).await);
    }

    #[tokio::test]
    async fn wait_for_rising_edge() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

        // known low
        assert!(!will_resolve(pin.wait_for_rising_edge()).await);
        // switched to high
        s.pullup(offset).expect("pullup should succeed");
        assert!(will_resolve(pin.wait_for_rising_edge()).await);
        assert!(!will_resolve(pin.wait_for_rising_edge()).await);
        // switched to low
        s.pulldown(offset).expect("pulldown should succeed");
        assert!(!will_resolve(pin.wait_for_rising_edge()).await);
        // switched to high
        s.pullup(offset).expect("pullup should succeed");
        assert!(will_resolve(pin.wait_for_rising_edge()).await);
        assert!(!will_resolve(pin.wait_for_rising_edge()).await);
    }

    #[tokio::test]
    async fn wait_for_falling_edge() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

        // known low
        assert!(!will_resolve(pin.wait_for_falling_edge()).await);
        // switched to high
        s.pullup(offset).expect("pullup should succeed");
        wait_propagation_delay();
        assert!(!will_resolve(pin.wait_for_falling_edge()).await);
        // switched to low
        s.pulldown(offset).expect("pulldown should succeed");
        assert!(will_resolve(pin.wait_for_falling_edge()).await);
        assert!(!will_resolve(pin.wait_for_falling_edge()).await);
        // switched to high
        s.pullup(offset).expect("pullup should succeed");
        assert!(!will_resolve(pin.wait_for_falling_edge()).await);
    }

    #[tokio::test]
    async fn wait_for_any_edge() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

        // known low
        assert!(!will_resolve(pin.wait_for_any_edge()).await);
        // switched to high
        s.pullup(offset).expect("pullup should succeed");
        assert!(will_resolve(pin.wait_for_any_edge()).await);
        // switched to low
        s.pulldown(offset).expect("pulldown should succeed");
        assert!(will_resolve(pin.wait_for_any_edge()).await);
        assert!(!will_resolve(pin.wait_for_any_edge()).await);
        // switched to high
        s.pullup(offset).expect("pullup should succeed");
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
        let pin = SyncInputPin::new(s.dev_path(), offset).expect("pin should exist");
        let mut pin = InputPin::from(pin);

        assert!(pin.is_low().expect("is_low should succeed"));
        s.pullup(offset).expect("pullup should succeed");
        wait_propagation_delay();
        assert!(pin.is_high().expect("is_high should succeed"));

        let req = gpiocdev::Request::from(pin);
        let config = req.config();
        let offsets = config.lines();
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0], offset);
        let line_config = config
            .line_config(offset)
            .expect("line_config should succeed");
        assert_eq!(line_config.direction, Some(Direction::Input));
        assert_eq!(line_config.edge_detection, None);
    }

    #[test]
    fn into_input() {
        let s = Simpleton::new(5);

        let offset = 3;
        let pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");
        let mut pin = SyncInputPin::from(pin);

        assert!(pin.is_low().expect("is_low should succeed"));
        s.pullup(offset).expect("pullup should succeed");
        wait_propagation_delay();
        assert!(pin.is_high().expect("is_high should succeed"));

        let req = gpiocdev::Request::from(pin);
        let config = req.config();
        let offsets = config.lines();
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0], offset);
        let line_config = config
            .line_config(offset)
            .expect("line_config should succeed");
        assert_eq!(line_config.direction, Some(Direction::Input));
        assert_eq!(line_config.edge_detection, None);
    }

    #[test]
    fn wait_for_high() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

        block_on(async {
            // known low
            assert!(!will_resolve(pin.wait_for_high()).await);
            assert!(will_resolve(pin.wait_for_low()).await);
            // switched to high
            s.pullup(offset).expect("pullup should succeed");
            assert!(will_resolve(pin.wait_for_high()).await);
            assert!(!will_resolve(pin.wait_for_low()).await);
            assert!(will_resolve(pin.wait_for_high()).await);
            // switched to low
            s.pulldown(offset).expect("pulldown should succeed");
            assert!(will_resolve(pin.wait_for_low()).await);
            assert!(!will_resolve(pin.wait_for_high()).await);
        })
    }

    #[test]
    fn wait_for_low() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

        block_on(async {
            // known low
            assert!(will_resolve(pin.wait_for_low()).await);
            // switched to high
            s.pullup(offset).expect("pullup should succeed");
            wait_propagation_delay();
            // Possible quirk - impl doesn't check for events if the last known level
            // is what you asked for - it does not check the hardware and so doesn't see
            // high until you explicitly check for high...
            // Safe to assume anyone using level logic will always be flip-flopping?
            assert!(will_resolve(pin.wait_for_low()).await);
            assert!(will_resolve(pin.wait_for_high()).await);
            assert!(!will_resolve(pin.wait_for_low()).await);
            // switched to low
            s.pulldown(offset).expect("pulldown should succeed");
            assert!(will_resolve(pin.wait_for_low()).await);
            assert!(!will_resolve(pin.wait_for_high()).await);
        })
    }

    #[test]
    fn wait_for_rising_edge() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

        block_on(async {
            // known low
            assert!(!will_resolve(pin.wait_for_rising_edge()).await);
            // switched to high
            s.pullup(offset).expect("pullup should succeed");
            assert!(will_resolve(pin.wait_for_rising_edge()).await);
            assert!(!will_resolve(pin.wait_for_rising_edge()).await);
            // switched to low
            s.pulldown(offset).expect("pulldown should succeed");
            assert!(!will_resolve(pin.wait_for_rising_edge()).await);
            // switched to high
            s.pullup(offset).expect("pullup should succeed");
            assert!(will_resolve(pin.wait_for_rising_edge()).await);
            assert!(!will_resolve(pin.wait_for_rising_edge()).await);
        })
    }

    #[test]
    fn wait_for_falling_edge() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

        block_on(async {
            // known low
            assert!(!will_resolve(pin.wait_for_falling_edge()).await);
            // switched to high
            s.pullup(offset).expect("pullup should succeed");
            wait_propagation_delay();
            assert!(!will_resolve(pin.wait_for_falling_edge()).await);
            // switched to low
            s.pulldown(offset).expect("pulldown should succeed");
            assert!(will_resolve(pin.wait_for_falling_edge()).await);
            assert!(!will_resolve(pin.wait_for_falling_edge()).await);
            // switched to high
            s.pullup(offset).expect("pullup should succeed");
            assert!(!will_resolve(pin.wait_for_falling_edge()).await);
        })
    }

    #[test]
    fn wait_for_any_edge() {
        let s = Simpleton::new(5);

        let offset = 3;
        let mut pin = InputPin::new(s.dev_path(), offset).expect("pin should exist");

        block_on(async {
            // known low
            assert!(!will_resolve(pin.wait_for_any_edge()).await);
            // switched to high
            s.pullup(offset).expect("pullup should succeed");
            assert!(will_resolve(pin.wait_for_any_edge()).await);
            // switched to low
            s.pulldown(offset).expect("pulldown should succeed");
            assert!(will_resolve(pin.wait_for_any_edge()).await);
            assert!(!will_resolve(pin.wait_for_any_edge()).await);
            // switched to high
            s.pullup(offset).expect("pullup should succeed");
            assert!(will_resolve(pin.wait_for_any_edge()).await);
            assert!(!will_resolve(pin.wait_for_any_edge()).await);
        });
    }
}
