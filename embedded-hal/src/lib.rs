// SPDX-FileCopyrightText: 2024 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A library providing [`embedded_hal::digital`] traits for
//! [`gpiocdev::Request`] and therefore for any Linux platform supporting the
//! GPIO character device.
//!
//! The requests contain only a single pin which must be configured as an
//! input or output.
//!
//! Asynchronous wrappers providing [`embedded_hal_async::digital::Wait`]
//! traits are available for various async reactors.
//!
//! The library can also be used to provide a simplified interface to
//! [`gpiocdev`] for simple use cases.
//!
//! # Example Usage
//!
//! Reading an input pin:
//!
//! ```no_run
//! # fn example() -> Result<(), gpiocdev_embedded_hal::Error> {
//! use embedded_hal::digital::InputPin;
//!
//! let mut pin = gpiocdev_embedded_hal::InputPin::new("/dev/gpiochip0", 4)?;
//! if pin.is_high()? {
//!     println!("Input is high.");
//! }
//! # Ok(())
//! # }
//! ```
//! Setting an output pin:
//!
//! ```no_run
//! # fn example() -> Result<(), gpiocdev_embedded_hal::Error> {
//! use embedded_hal::digital::{OutputPin, PinState};
//!
//! // level is set as part of the request
//! let mut led0 = gpiocdev_embedded_hal::OutputPin::from_name("LED0", PinState::High)?;
//!
//! // change the level later
//! led0.set_low()?;
//! # Ok(())
//! # }
//! ```
//!
//! Waiting for edges on an input pin:
//!
//!  ```no_run
//! # #[cfg(feature = "async_tokio")]
//! # async fn example() -> Result<(), gpiocdev_embedded_hal::Error> {
//! use embedded_hal::digital::InputPin;
//! use embedded_hal_async::digital::Wait;
//!
//! let mut pin = gpiocdev_embedded_hal::tokio::InputPin::new("/dev/gpiochip0", 4)?;
//!
//! pin.wait_for_any_edge().await?;
//! if pin.is_high()? {
//!     println!("Input is high.");
//! }
//! # Ok(())
//! # }
//! ```
use std::path::Path;

use embedded_hal::digital::PinState;
use gpiocdev::{
    line::{Config, Direction, Offset, Value},
    Request,
};

/// Wrappers for various async reactors.
#[cfg(any(feature = "async_tokio", feature = "async_io"))]
mod r#async;

#[cfg(feature = "async_io")]
pub use r#async::async_io;
#[cfg(feature = "async_tokio")]
pub use r#async::tokio;

/// Core common class for InputPin and OutputPin.
#[derive(Debug)]
struct Pin {
    req: Request,
    offset: Offset,
    config: Config,
}

impl Pin {
    #[inline]
    fn is_high(&mut self) -> Result<bool, Error> {
        Ok(self.req.as_ref().value(self.offset)?
            == state_to_value(PinState::High, self.config.active_low))
    }

    #[inline]
    fn is_low(&mut self) -> Result<bool, Error> {
        Ok(!self.is_high()?)
    }
}

impl From<Pin> for Request {
    fn from(pin: Pin) -> Self {
        pin.req
    }
}

/// Provides [`embedded_hal::digital`] traits for a [`gpiocdev::Request`]
/// containing a single input pin.
///
/// Holding the [`InputPin`] grants exclusive access to the pin.
///
/// Do NOT drop the [`InputPin`] until you are completely done with it.
/// Dropping and re-requesting the line is far more expensive than getting the
/// value.
#[derive(Debug)]
pub struct InputPin(Pin);

impl InputPin {
    /// Creates a new input pin for the given `offset` on the given `chip`.
    ///
    /// ```no_run
    /// # fn example() -> Result<(), gpiocdev_embedded_hal::Error> {
    /// use embedded_hal::digital::InputPin;
    ///
    /// let mut pin = gpiocdev_embedded_hal::InputPin::new("/dev/gpiochip0", 4)?;
    /// if pin.is_high()? {
    ///     println!("Input is high.");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn new<P>(chip: P, offset: u32) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        let req = Request::builder()
            .on_chip(chip.as_ref())
            .with_line(offset)
            .as_input()
            .request()?;

        InputPin::try_from(req)
    }

    /// Set this pin to output mode.
    pub fn into_output_pin(mut self, state: PinState) -> Result<OutputPin, Error> {
        let pin = &mut self.0;
        let req = pin.req.as_ref();
        let value = state_to_value(state, pin.config.active_low);
        let mut config = req.config();
        config.from_line_config(&pin.config).as_output(value);
        req.reconfigure(&config)?;
        // don't update the whole config - retain the input specific fields
        // (edge_detection and debounce) in case the pin is switched to input.
        pin.config.direction = Some(Direction::Output);
        pin.config.value = Some(value);

        Ok(OutputPin(self.0))
    }

    /// Create an [`InputPin`] from a [`gpiocdev::FoundLine`].
    ///
    /// # Examples
    /// ```no_run
    /// # fn example() -> Result<(), gpiocdev_embedded_hal::Error> {
    /// use embedded_hal::digital::InputPin;
    ///
    /// let sensor0 = gpiocdev::find_named_line("SENSOR0").unwrap();
    /// let mut pin = gpiocdev_embedded_hal::InputPin::from_found_line(sensor0)?;
    /// if pin.is_low()? {
    ///     println!("Input is low.");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_found_line(fl: gpiocdev::FoundLine) -> Result<Self, Error> {
        let req = Request::builder()
            .with_found_line(&fl)
            .as_input()
            .request()?;
        let config = req.config();
        let line_config = config.line_config(fl.info.offset).unwrap().clone();
        Ok(InputPin(Pin {
            req,
            offset: fl.info.offset,
            config: line_config,
        }))
    }

    /// Create an [`InputPin`] given a line name.
    ///
    /// # Examples
    /// ```no_run
    /// # fn example() -> Result<(), gpiocdev_embedded_hal::Error> {
    /// use embedded_hal::digital::InputPin;
    ///
    /// let mut gpio22 = gpiocdev_embedded_hal::InputPin::from_name("GPIO22")?;
    /// if gpio22.is_low()? {
    ///     println!("Input is low.");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_name(name: &str) -> Result<Self, Error> {
        let line = gpiocdev::find_named_line(name)
            .ok_or_else(|| Error::UnfoundLine(name.into()))?;
        Self::from_found_line(line)
    }
}

impl TryFrom<Request> for InputPin {
    type Error = Error;

    /// Convert any single input line [`gpiocdev::Request`] into a [`InputPin`].
    ///
    /// This allows for advanced configurations such as setting bias
    /// or using the active_low flag to flip the line polarity.
    /// [`InputPin::new<P>()`] should be used for less complex configurations.
    fn try_from(req: Request) -> Result<Self, Self::Error> {
        let config = req.as_ref().config();
        let offsets = config.lines();
        if offsets.len() != 1 {
            return Err(Error::MultipleLinesRequested);
        }
        let offset = offsets[0];
        // unwrap is safe as line config must exist.
        let line_config = config.line_config(offset).unwrap().clone();
        if line_config.direction != Some(Direction::Input) {
            return Err(Error::RequiresInputMode);
        }

        Ok(InputPin(Pin {
            req,
            offset,
            config: line_config,
        }))
    }
}

impl From<InputPin> for Request {
    /// Convert the [`InputPin`] into the contained [`Request`].
    fn from(pin: InputPin) -> Self {
        pin.0.req
    }
}

impl embedded_hal::digital::InputPin for InputPin {
    #[inline]
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        self.0.is_high()
    }

    #[inline]
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.0.is_low()
    }
}

impl embedded_hal::digital::ErrorType for InputPin {
    /// Errors returned by [`InputPin`].
    type Error = Error;
}

/// Provides [`embedded_hal::digital`] traits for a [`gpiocdev::Request`]
/// containing a single output pin.
///
/// Holding the [`OutputPin`] grants exclusive access to the pin.
///
/// Do NOT drop the [`OutputPin`] until you are completely done with it.
/// While you hold the [`OutputPin`] the line is guaranteed to remain as set,
/// but when dropped it may be altered, either by other users or by the kernel
/// itself.
/// Dropping and re-requesting the line is also far more expensive than setting
/// the value.
#[derive(Debug)]
pub struct OutputPin(Pin);

impl OutputPin {
    /// Creates a new output pin for the given `offset` on the given `chip`.
    ///
    /// ```no_run
    /// # fn example() -> Result<(), gpiocdev_embedded_hal::Error> {
    /// use embedded_hal::digital::{OutputPin, PinState};
    ///
    /// let mut pin = gpiocdev_embedded_hal::OutputPin::new("/dev/gpiochip0", 17, PinState::Low)?;
    /// // later...
    /// pin.set_high()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new<P>(chip: P, offset: u32, state: PinState) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        let req = Request::builder()
            .on_chip(chip.as_ref())
            .with_line(offset)
            .as_output(state_to_value(state, false))
            .request()?;

        OutputPin::try_from(req)
    }

    /// Set this pin to input mode.
    pub fn into_input_pin(mut self) -> Result<InputPin, Error> {
        let pin = &mut self.0;
        let req = pin.req.as_ref();
        let mut config = req.config();
        config.from_line_config(&pin.config).as_input();
        req.reconfigure(&config)?;
        // don't update the whole config - retain the output specific fields
        // (drive) in case the pin is switched back to output.
        pin.config.direction = Some(Direction::Input);
        pin.config.value = None;

        Ok(InputPin(self.0))
    }

    /// Create an [`OutputPin`] from a [`gpiocdev::FoundLine`].
    ///
    /// # Examples
    /// ```no_run
    /// # fn example() -> Result<(), gpiocdev_embedded_hal::Error> {
    /// use embedded_hal::digital::{OutputPin, PinState};
    ///
    /// let led0 = gpiocdev::find_named_line("LED0").unwrap();
    /// let mut pin = gpiocdev_embedded_hal::OutputPin::from_found_line(led0, PinState::High)?;
    /// // ...
    /// pin.set_low()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_found_line(fl: gpiocdev::FoundLine, state: PinState) -> Result<Self, Error> {
        let req = Request::builder()
            .with_found_line(&fl)
            .as_output(state_to_value(state, false))
            .request()?;
        let config = req.config();
        let line_config = config.line_config(fl.info.offset).unwrap().clone();
        Ok(OutputPin(Pin {
            req,
            offset: fl.info.offset,
            config: line_config,
        }))
    }

    /// Create an [`OutputPin`] given a line name.
    ///
    /// # Examples
    /// ```no_run
    /// use embedded_hal::digital::{OutputPin, PinState};
    ///
    /// # fn example() -> Result<(), gpiocdev_embedded_hal::Error> {
    /// let mut led0 = gpiocdev_embedded_hal::OutputPin::from_name("LED0", PinState::High)?;
    /// // ...
    /// led0.set_low()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_name(name: &str, state: PinState) -> Result<Self, Error> {
        let line = gpiocdev::find_named_line(name)
            .ok_or_else(|| Error::UnfoundLine(name.into()))?;
        Self::from_found_line(line, state)
    }
}

impl TryFrom<Request> for OutputPin {
    type Error = Error;

    /// Convert any single output line [`gpiocdev::Request`] into an [`OutputPin`].
    ///
    /// This allows for advanced configurations such as setting bias or drive
    /// or using the active_low flag to flip the line polarity.
    /// [`OutputPin::new<P>()`] should be used for less complex configurations.
    fn try_from(req: Request) -> Result<Self, Self::Error> {
        let config = req.as_ref().config();
        let offsets = config.lines();
        if offsets.len() != 1 {
            return Err(Error::MultipleLinesRequested);
        }
        let offset = offsets[0];
        // unwrap is safe as line config must exist.
        let line_config = config.line_config(offset).unwrap().clone();
        if line_config.direction != Some(Direction::Output) {
            return Err(Error::RequiresOutputMode);
        }

        Ok(OutputPin(Pin {
            req,
            offset,
            config: line_config,
        }))
    }
}

impl From<OutputPin> for Request {
    /// Convert the [`OutputPin`] into the contained [`Request`].
    fn from(pin: OutputPin) -> Self {
        pin.0.req
    }
}

impl embedded_hal::digital::InputPin for OutputPin {
    // Supporting InputPin is intentional to allow support for reading the
    // physical line value for output lines where supported by hardware,
    // e.g. to read an open drain line while not actively driving it.

    #[inline]
    /// Is the line high?
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        self.0.is_high()
    }

    #[inline]
    /// Is the line low?
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.0.is_low()
    }
}

impl embedded_hal::digital::OutputPin for OutputPin {
    #[inline]
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.set_state(PinState::Low)
    }

    #[inline]
    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.set_state(PinState::High)
    }

    fn set_state(&mut self, state: PinState) -> Result<(), Error> {
        let pin = &mut self.0;
        let value = state_to_value(state, pin.config.active_low);
        if pin.config.value != Some(value) {
            pin.req.as_ref().set_value(pin.offset, value)?;
            pin.config.value = Some(value);
        }
        Ok(())
    }
}

impl embedded_hal::digital::StatefulOutputPin for OutputPin {
    fn is_set_high(&mut self) -> Result<bool, Self::Error> {
        Ok(self.0.config.value == Some(Value::Active))
    }

    fn is_set_low(&mut self) -> Result<bool, Self::Error> {
        Ok(self.0.config.value == Some(Value::Inactive))
    }

    fn toggle(&mut self) -> Result<(), Self::Error> {
        let pin = &mut self.0;
        let value = pin.config.value.unwrap_or_default().not();
        pin.req.as_ref().set_value(pin.offset, value)?;
        pin.config.value = Some(value);
        Ok(())
    }
}

impl embedded_hal::digital::ErrorType for OutputPin {
    /// Errors returned by [`OutputPin`].
    type Error = Error;
}

/// Converts a [`PinState`] to the gpiocdev logical line [`Value`].
fn state_to_value(state: PinState, is_active_low: bool) -> Value {
    let value = match state {
        PinState::High => Value::Active,
        PinState::Low => Value::Inactive,
    };
    if is_active_low {
        return value.not();
    }
    value
}

/// Errors returned by [`gpiocdev_embedded_hal`](crate) types.
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum Error {
    /// Requests can only contain a single requested line.
    #[error("Request must not contain a multiple lines")]
    MultipleLinesRequested,

    /// InputPins must be in input mode.
    #[error("Requested pin must be in input mode")]
    RequiresInputMode,

    /// OutputPins must be in output mode.
    #[error("Requested pin must be in output mode")]
    RequiresOutputMode,

    /// Cannot find named line.
    #[error("Cannot find a line named '{0}'")]
    UnfoundLine(String),

    /// An error returned from an underlying gpiocdev call.
    #[error("gpiocdev returned: {0}")]
    Cdev(#[source] gpiocdev::Error),
}

impl From<gpiocdev::Error> for Error {
    fn from(err: gpiocdev::Error) -> Self {
        Self::Cdev(err)
    }
}

impl embedded_hal::digital::Error for Error {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        embedded_hal::digital::ErrorKind::Other
    }
}
