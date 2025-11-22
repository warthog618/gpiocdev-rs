// SPDX-FileCopyrightText: 2024 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use crate::{state_to_value, Error};
use embedded_hal::digital::PinState;
use gpiocdev::async_io::AsyncRequest;
use gpiocdev::line::{Config, EdgeDetection, EdgeKind, Offset, Value};
use gpiocdev::Request;

/// Provides the [`embedded_hal::digital`] and [`embedded_hal_async::digital::Wait`]
/// traits for a [`gpiocdev::Request`] containing a single input pin using the
/// async-io reactor.
///
/// Holding the [`InputPin`] grants exclusive access to the pin.
///
/// Do NOT drop the [`InputPin`] until you are completely done with it.
/// Dropping and re-requesting the line is far more expensive than getting the value.
pub struct InputPin {
    req: AsyncRequest,
    offset: Offset,
    config: Config,
}

impl InputPin {
    /// Creates a new input pin for the given `offset` on the given `chip`.
    ///
    /// ```no_run
    /// use embedded_hal::digital::InputPin;
    /// use embedded_hal_async::digital::Wait;
    /// # use gpiocdev_embedded_hal::Error;
    ///
    /// # async fn example() -> Result<(), Error> {
    /// let mut pin = gpiocdev_embedded_hal::async_io::InputPin::new("/dev/gpiochip0", 4)?;
    /// if pin.is_high()? {
    ///     println!("Input is high.");
    /// }
    /// pin.wait_for_falling_edge().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new<P>(chip: P, offset: u32) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        Ok(crate::InputPin::new(chip, offset)?.into())
    }

    #[inline]
    fn is_high(&mut self) -> Result<bool, Error> {
        Ok(self.req.as_ref().value(self.offset)?
            == state_to_value(PinState::High, self.config.active_low))
    }

    #[inline]
    fn is_low(&mut self) -> Result<bool, Error> {
        Ok(!self.is_high()?)
    }

    /// Set this pin to output mode.
    pub fn into_output_pin(self, state: PinState) -> Result<crate::OutputPin, Error> {
        let pin: crate::InputPin = self.into();
        pin.into_output_pin(state)
    }

    /// The value of the line following the most recent edge.
    fn last_value(&mut self) -> Result<Value, Error> {
        if self.config.value.is_none() {
            self.config.value = Some(self.req.as_ref().value(self.offset)?);
        }
        Ok(self.config.value.expect("value should be initialised"))
    }

    async fn wait_for_edge(&mut self, edge: EdgeDetection) -> Result<(), Error> {
        self.enable_edge_detection(edge)?;
        loop {
            let event = self.req.read_edge_event().await?;

            if match edge {
                EdgeDetection::BothEdges => true,
                EdgeDetection::RisingEdge => event.kind == EdgeKind::Rising,
                EdgeDetection::FallingEdge => event.kind == EdgeKind::Falling,
            } {
                let value = match event.kind {
                    EdgeKind::Rising => Value::Active,
                    EdgeKind::Falling => Value::Inactive,
                };
                self.config.value = Some(value);
                return Ok(());
            }
        }
    }

    async fn wait_for_level(&mut self, value: Value) -> Result<(), Error> {
        let edge = match value {
            Value::Active => EdgeDetection::RisingEdge,
            Value::Inactive => EdgeDetection::FallingEdge,
        };
        self.enable_edge_detection(edge)?;
        if self.last_value()? == value {
            return Ok(());
        }
        loop {
            let event = self.req.read_edge_event().await?;

            let nv = match event.kind {
                EdgeKind::Rising => Value::Active,
                EdgeKind::Falling => Value::Inactive,
            };
            self.config.value = Some(nv);
            if value == nv {
                return Ok(());
            }
        }
    }
    /// Ensure that the request configuration has edge detection enabled for the requested edge.
    fn enable_edge_detection(&mut self, edge: EdgeDetection) -> Result<(), Error> {
        let new_detection = match self.config.edge_detection {
            Some(EdgeDetection::BothEdges) => return Ok(()),
            Some(x) => {
                if x == edge {
                    return Ok(());
                };
                EdgeDetection::BothEdges
            }
            None => edge,
        };
        let req = self.req.as_ref();
        req.reconfigure(req.config().with_edge_detection(new_detection))?;
        self.config.edge_detection = Some(new_detection);
        // force reading of current value
        self.config.value = None;
        Ok(())
    }
}

impl From<InputPin> for Request {
    /// Convert the [`InputPin`] into the contained [`Request`].
    fn from(pin: InputPin) -> Self {
        pin.req.into()
    }
}

impl From<crate::InputPin> for InputPin {
    /// Convert a synchronous [`InputPin`](crate::InputPin) into an async_io [`InputPin`].
    fn from(pin: crate::InputPin) -> Self {
        InputPin {
            req: pin.0.req.into(),
            offset: pin.0.offset,
            config: pin.0.config,
        }
    }
}

impl From<InputPin> for crate::InputPin {
    /// Convert an async_io [`InputPin`] into a synchronous [`InputPin`](crate::InputPin).
    fn from(pin: InputPin) -> Self {
        crate::InputPin(crate::Pin {
            req: pin.req.into(),
            offset: pin.offset,
            config: pin.config,
        })
    }
}

impl embedded_hal::digital::ErrorType for InputPin {
    /// Errors returned by the [`InputPin`].
    type Error = Error;
}

impl embedded_hal::digital::InputPin for InputPin {
    #[inline]
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        self.is_high()
    }

    #[inline]
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.is_low()
    }
}

impl embedded_hal_async::digital::Wait for InputPin {
    /// Wait for the pin to go high.
    ///
    /// # Note
    /// The implementation is event driven, and only looks for events when waiting
    /// for the level to change.
    /// If the returned future resolves then subsequent calls are effectively a no-op.
    /// Wait on an edge or the pin to go low instead.
    #[inline]
    async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
        self.wait_for_level(Value::Active).await
    }

    /// Wait for the pin to go low.
    ///
    /// # Note
    /// The implementation is event driven, and only looks for events when waiting
    /// for the level to change.
    /// If the returned future resolves then subsequent calls are effectively a no-op.
    /// Wait on an edge or the pin to go high instead.
    #[inline]
    async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
        self.wait_for_level(Value::Inactive).await
    }

    #[inline]
    async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
        self.wait_for_edge(EdgeDetection::RisingEdge).await
    }

    #[inline]
    async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
        self.wait_for_edge(EdgeDetection::FallingEdge).await
    }

    #[inline]
    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        self.wait_for_edge(EdgeDetection::BothEdges).await
    }
}
