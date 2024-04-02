/// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT
use crate::chip::Chip;
use crate::line::{self, Bias, Direction, Drive, EdgeDetection, EventClock, Offset, Value, Values};
use crate::request::{Config, Request};
#[cfg(feature = "uapi_v1")]
use crate::AbiVersion;
use crate::{Error, Result, UapiCall};
#[cfg(feature = "uapi_v1")]
use gpiocdev_uapi::v1;
#[cfg(feature = "uapi_v2")]
use gpiocdev_uapi::v2;
use gpiocdev_uapi::NUM_LINES_MAX;
use std::cmp::max;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// A builder of line requests.
///
/// Apply mutators to specify the request configuration, then use [`request`]
/// to request the lines from the kernel.
///
/// Most mutators operate on the request configuration as per the [`Config`].
///
/// # Examples
/// Request and read a basic input line:
/// ```no_run
/// # fn example() -> Result<(), gpiocdev::Error> {
/// let l3 = gpiocdev::Request::builder()
///     .on_chip("/dev/gpiochip0")
///     .with_line(3)
///     .request()?;
/// let value = l3.value(3)?;
/// # Ok(())
/// # }
/// ```
///
/// Several lines in one request:
///
/// ```no_run
/// # use gpiocdev::line::Values;
/// # fn example() -> Result<(), gpiocdev::Error> {
/// let offsets = &[3,5];
/// let req = gpiocdev::Request::builder()
///     .on_chip("/dev/gpiochip0")
///     .with_lines(offsets)
///     .request()?;
/// let mut values = Values::from_offsets(offsets);
/// req.values(&mut values)?;
/// # Ok(())
/// # }
/// ```
///
/// More complex configurations can be built separately and provided
/// to the `Builder`:
///
/// ```no_run
/// # use gpiocdev::request::Config;
/// # use gpiocdev::line::Value;
/// # fn example() -> Result<(), gpiocdev::Error> {
/// let mut cfg = Config::default();
/// cfg.with_line(5).as_output(Value::Active);
/// let req = gpiocdev::Request::from_config(cfg)
///     .on_chip("/dev/gpiochip0")
///     .request()?;
/// req.set_value(5,Value::Inactive)?;
/// # Ok(())
/// # }
/// ```
///
/// [`request`]: #method.request
#[derive(Clone, Default, Eq, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub struct Builder {
    pub(super) cfg: Config,
    pub(super) consumer: String,
    pub(super) kernel_event_buffer_size: u32,
    pub(super) user_event_buffer_size: usize,
    err: Option<Error>,
    /// The ABI version used to create the request, and so determines how to decode events.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    pub(super) abiv: Option<AbiVersion>,
}

impl Builder {
    /// Start building a new request using the provided config.
    pub(crate) fn from_config(cfg: Config) -> Self {
        Builder {
            cfg,
            ..Default::default()
        }
    }

    /// Perform the request.
    ///
    /// Sends the request to the kernel using the appropriate uAPI call.
    ///
    /// On success returns the [`Request`] that provides access to the requested lines.
    pub fn request(&mut self) -> Result<Request> {
        if let Some(e) = &self.err {
            return Err(e.clone());
        }
        if self.cfg.chip.as_os_str().is_empty() {
            return Err(Error::InvalidArgument("No chip specified.".into()));
        }
        let chip = Chip::from_path(&self.cfg.chip)?;
        self.cfg.offsets.sort_unstable();
        self.do_request(&chip).map(|f| self.to_request(f))
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_request(&mut self, chip: &Chip) -> Result<File> {
        if self.abiv.is_none() {
            self.abiv = Some(chip.detect_abi_version()?);
        }
        match self.to_uapi()? {
            UapiRequest::Handle(hr) => v1::get_line_handle(&chip.f, hr)
                .map_err(|e| Error::Uapi(UapiCall::GetLineHandle, e)),
            UapiRequest::Event(er) => {
                v1::get_line_event(&chip.f, er).map_err(|e| Error::Uapi(UapiCall::GetLineEvent, e))
            }
            UapiRequest::Line(lr) => {
                v2::get_line(&chip.f, lr).map_err(|e| Error::Uapi(UapiCall::GetLine, e))
            }
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_request(&self, chip: &Chip) -> Result<File> {
        match self.to_uapi()? {
            UapiRequest::Handle(hr) => v1::get_line_handle(&chip.f, hr)
                .map_err(|e| Error::Uapi(UapiCall::GetLineHandle, e)),
            UapiRequest::Event(er) => {
                v1::get_line_event(&chip.f, er).map_err(|e| Error::Uapi(UapiCall::GetLineEvent, e))
            }
        }
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_request(&self, chip: &Chip) -> Result<File> {
        match self.to_uapi()? {
            UapiRequest::Line(lr) => {
                v2::get_line(&chip.f, lr).map_err(|e| Error::Uapi(UapiCall::GetLine, e))
            }
        }
    }

    fn to_request(&self, f: File) -> Request {
        Request {
            f,
            offsets: self.cfg.offsets.clone(),
            cfg: Arc::new(RwLock::new(self.cfg.clone())),
            user_event_buffer_size: max(self.user_event_buffer_size, 1),
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            abiv: self.abiv.unwrap(),
        }
    }

    /// Replace the request configuration with the new one provided.
    pub fn with_config(&mut self, cfg: Config) -> &mut Self {
        self.cfg = cfg;
        self
    }

    /// Get a snapshot of the current request configuration.
    ///
    /// Can be applied to other requests with [`with_config`] or [`Request::from_config`].
    ///
    /// [`with_config`]: #method.with_config
    /// [`Request.from_config`]: struct.Request.html#method.from_config
    pub fn config(&self) -> Config {
        self.cfg.clone()
    }

    // mutators for attributes that cannot be changed after the request is made.

    /// Specify the consumer label to be applied to the request, and so to all lines
    /// in the request.
    ///
    /// If not specified, a label *"gpiocdev-p**PID**"* is applied by [`request`],
    /// where **PID** is the process id of the application.
    ///
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::{Request, Result};
    /// # use gpiocdev::line::Values;
    /// # fn example() -> Result<()> {
    /// let req = Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_lines(&[3,5])
    ///     .with_consumer("spice_weasel")
    ///     .request()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`request`]: #method.request
    pub fn with_consumer<N: Into<String>>(&mut self, consumer: N) -> &mut Self {
        self.consumer = consumer.into();
        self
    }

    /// Set the event buffer size for edge events buffered in the kernel.
    ///
    /// This method is only required in unusual circumstances.
    ///
    /// The size provides a hint to the kernel as to the size of the buffer to store edge
    /// events until they are read from the Request.
    /// The default size is 16 times the number of lines in the request.
    /// The default size is typically sufficient unless the hardware may generate bursts
    /// of events faster than the application can service them.
    ///
    /// Altering the buffer size does NOT affect latency.
    /// Buffering is provided in the kernel to reduce the likelihood of event loss when
    /// user space is slow servicing events.
    /// In all cases the events are provided to user space as quickly as user space allows.
    pub fn with_kernel_event_buffer_size(&mut self, event_buffer_size: u32) -> &mut Self {
        self.kernel_event_buffer_size = event_buffer_size;
        self
    }

    /// Set the event buffer size for edge events buffered in user space.
    ///
    /// This method is only required in unusual circumstances.
    ///
    /// The size defines the number of events that may be buffered in user space by the
    /// [`Request.edge_events`] iterator.  The user space buffer is a performance optimisation to
    /// reduce the number of system calls required to read events.
    ///
    /// Altering the buffer size does NOT affect latency.
    /// In all cases the events are provided to user space as quickly as user space allows.
    ///
    /// [`Request.edge_events`]: struct.Request.html#method.edge_events
    pub fn with_user_event_buffer_size(&mut self, event_buffer_size: usize) -> &mut Self {
        self.user_event_buffer_size = event_buffer_size;
        self
    }

    /// Select the ABI version to use when requesting the lines and for subsequent operations.
    ///
    /// This is not normally required - the library will determine the available ABI versions
    /// and use the latest.
    ///
    /// This is only required where an application supports both uAPI versions but wishes
    /// to force the usage of a particular version at runtime, bypassing the automatic
    /// selection process.
    ///
    /// The version can be tested for availability using [`supports_abi_version`].
    ///
    /// [`supports_abi_version`]: fn@crate::supports_abi_version
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    pub fn using_abi_version(&mut self, abiv: AbiVersion) -> &mut Self {
        self.abiv = Some(abiv);
        self
    }

    /// Set the chip from which to request lines.
    ///
    /// This applies to all lines in the request. It is not possible to request lines
    /// from different chips in the same request.
    ///
    /// The `Builder` locks to the first chip provided to it, for example by [`on_chip`]
    /// or [`with_found_line`].
    /// Any subsequent attempts to change the chip will result in an error when [`request`] is called.
    ///
    /// The chip is identified by a path which must resolve to a GPIO character device.
    ///
    /// [`on_chip`]: #method.on_chip
    /// [`with_found_line`]: #method.with_found_line
    /// [`request`]: #method.request
    pub fn on_chip<P: Into<PathBuf>>(&mut self, path: P) -> &mut Self {
        if self.cfg.chip.as_os_str().is_empty() {
            self.cfg.on_chip(path);
        } else if self.cfg.chip != path.into() {
            self.err = Some(Error::InvalidArgument("Multiple chips requested.".into()))
        }
        self
    }

    // Pass through mutators to the contained Config

    /// Set the selected lines to input.
    ///
    /// This is a short form of [`with_direction(Input)`].
    ///
    /// This is the default direction setting.
    ///
    /// [`with_direction(Input)`]: #method.with_direction
    pub fn as_input(&mut self) -> &mut Self {
        self.cfg.as_input();
        self
    }

    /// Set the selected lines to output with the given value.
    ///
    /// This is a short form of [`with_direction(Output)`] and [`with_value(value)`]
    /// that allows the direction and value to be set in the one call.
    ///
    /// [`with_direction(Output)`]: #method.with_direction
    /// [`with_value(value)`]: #method.with_value
    pub fn as_output(&mut self, value: Value) -> &mut Self {
        self.cfg.as_output(value);
        self
    }

    /// Do not set the direction of the selected lines.
    pub fn as_is(&mut self) -> &mut Self {
        self.cfg.as_is();
        self
    }

    /// Set the selected lines to active low.
    pub fn as_active_low(&mut self) -> &mut Self {
        self.cfg.as_active_low();
        self
    }

    /// Set the selected lines to active high.
    ///
    /// This is the default active level setting.
    pub fn as_active_high(&mut self) -> &mut Self {
        self.cfg.as_active_high();
        self
    }

    /// Set the bias setting for the selected lines.
    pub fn with_bias<B: Into<Option<Bias>>>(&mut self, bias: B) -> &mut Self {
        self.cfg.with_bias(bias);
        self
    }

    /// Set the debounce period for the selected lines.
    ///
    /// A value of zero means no debounce.
    ///
    /// Implicitly sets the selected lines as inputs, if they weren't already, and
    /// removes any output specific settings.
    pub fn with_debounce_period(&mut self, period: Duration) -> &mut Self {
        self.cfg.with_debounce_period(period);
        self
    }

    /// Set the direction of the selected lines.
    ///
    /// Setting to input removes any output specific settings.
    ///
    /// Setting to output removes any input specific settings.
    pub fn with_direction(&mut self, direction: Direction) -> &mut Self {
        self.cfg.with_direction(direction);
        self
    }

    /// Set the drive setting for the selected lines.
    ///
    /// Implicitly sets the selected lines as outputs, if they weren't already, and
    /// removes any input specific settings.
    pub fn with_drive(&mut self, drive: Drive) -> &mut Self {
        self.cfg.with_drive(drive);
        self
    }

    /// Set the edge detection for the selected lines.
    ///
    /// Implicitly sets the lines as inputs and removes any output specific settings.
    pub fn with_edge_detection<E: Into<Option<EdgeDetection>>>(&mut self, edge: E) -> &mut Self {
        self.cfg.with_edge_detection(edge);
        self
    }

    /// Set the clock source for edge events on the selected lines.
    pub fn with_event_clock<E: Into<Option<EventClock>>>(&mut self, event_clock: E) -> &mut Self {
        self.cfg.with_event_clock(event_clock);
        self
    }

    /// Add a found line to the request.
    ///
    /// The line must be on the same chip as any existing lines in the request, else the line is
    /// ignored and an error returned when [`request`](#method.request) is called.
    ///
    /// Note that all configuration mutators applied subsequently only apply to this line.
    ///
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::line::Value;
    /// # fn example() -> Result<(), gpiocdev::Error> {
    /// let led0 = gpiocdev::find_named_line("LED0").unwrap();
    /// let req = gpiocdev::Request::builder()
    ///     .with_found_line(&led0)
    ///     .as_output(Value::Active)
    ///     .request()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_found_line(&mut self, line: &crate::FoundLine) -> &mut Self {
        if let Err(e) = self.cfg.with_found_line(line) {
            self.err = Some(e);
        }
        self
    }

    /// Add a set of found lines to the request.
    ///
    /// The lines must be on the same chip as any existing lines in the request, else the line is
    /// ignored and an error returned when [`request`](#method.request) is called.
    ///
    /// Note that all configuration mutators applied subsequently only apply to these lines.
    ///
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::line::EdgeDetection;
    /// # fn example() -> Result<(), gpiocdev::Error> {
    /// let buttons = gpiocdev::find_named_lines(&["BUTTON0","BUTTON1"], true)?;
    /// let req = gpiocdev::Request::builder()
    ///     .with_found_lines(&buttons)
    ///     .with_edge_detection(EdgeDetection::BothEdges)
    ///     .request()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_found_lines(&mut self, lines: &HashMap<&str, crate::FoundLine>) -> &mut Self {
        if let Err(e) = self.cfg.with_found_lines(lines) {
            self.err = Some(e);
        }
        self
    }

    /// Add a line to the request.
    ///
    /// Note that all configuration mutators applied subsequently only
    /// apply to this line.
    pub fn with_line(&mut self, offset: Offset) -> &mut Self {
        self.cfg.with_line(offset);
        self
    }

    /// Remove a line from the request.
    pub fn without_line(&mut self, offset: Offset) -> &mut Self {
        self.cfg.without_line(offset);
        self
    }

    /// Add a set of lines to the request.
    ///
    /// Note that all configuration mutators applied subsequently only
    /// apply to this subset of lines.
    pub fn with_lines(&mut self, offsets: &[Offset]) -> &mut Self {
        self.cfg.with_lines(offsets);
        self
    }

    /// Add a set of output lines, with values, to the selected lines.
    ///
    /// This is a short form of [`with_line(offset)`](#method.with_line) and
    /// [`as_output(value)`](#method.as_output), applied to a set of line values.
    pub fn with_output_lines(&mut self, values: &Values) -> &mut Self {
        self.cfg.with_output_lines(values);
        self
    }

    /// Remove a set of lines from the request.
    pub fn without_lines(&mut self, offsets: &[Offset]) -> &mut Self {
        self.cfg.without_lines(offsets);
        self
    }

    /// Set the value of the selected lines.
    pub fn with_value(&mut self, value: Value) -> &mut Self {
        self.cfg.with_value(value);
        self
    }

    /// Apply the configuration based on a snapshot from a single line.
    pub fn from_line_config(&mut self, cfg: &line::Config) -> &mut Self {
        self.cfg.from_line_config(cfg);
        self
    }

    // Conversions into uAPI types.
    fn to_uapi(&self) -> Result<UapiRequest> {
        if self.cfg.num_lines() == 0 {
            return Err(Error::InvalidArgument("No lines specified.".into()));
        }
        if self.cfg.offsets.len() > NUM_LINES_MAX {
            return Err(Error::InvalidArgument(format!(
                "Requested {} lines is greater than the maximum of {}.",
                self.cfg.offsets.len(),
                NUM_LINES_MAX,
            )));
        }
        self.do_to_uapi()
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_to_uapi(&self) -> Result<UapiRequest> {
        match self.abiv.unwrap() {
            AbiVersion::V1 => self.to_v1(),
            AbiVersion::V2 => self.to_v2(),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_to_uapi(&self) -> Result<UapiRequest> {
        self.to_v1()
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_to_uapi(&self) -> Result<UapiRequest> {
        self.to_v2()
    }

    #[cfg(feature = "uapi_v1")]
    fn to_v1(&self) -> Result<UapiRequest> {
        if self.kernel_event_buffer_size != 0 {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "does not support setting event buffer size".into(),
            ));
        }
        let lcfg = self.cfg.unique()?;
        if lcfg.debounce_period.is_some() {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "does not support debounce".into(),
            ));
        }
        if lcfg.event_clock.is_some() {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "does not support selecting the event clock source".into(),
            ));
        }
        let consumer = if self.consumer.is_empty() {
            default_consumer().as_str().into()
        } else {
            self.consumer.as_str().into()
        };
        if lcfg.edge_detection.is_some() {
            if self.cfg.offsets.len() != 1 {
                return Err(Error::AbiLimitation(
                    AbiVersion::V1,
                    "only supports edge detection on single line requests".into(),
                ));
            }
            Ok(UapiRequest::Event(v1::EventRequest {
                offset: self.cfg.offsets[0],
                handleflags: lcfg.into(),
                eventflags: lcfg.into(),
                consumer,
                ..Default::default()
            }))
        } else {
            Ok(UapiRequest::Handle(v1::HandleRequest {
                offsets: v1::Offsets::from_slice(&self.cfg.offsets),
                flags: lcfg.into(),
                values: self.cfg.to_v1_values()?,
                consumer,
                num_lines: self.cfg.offsets.len() as u32,
                ..Default::default()
            }))
        }
    }

    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    fn to_v2(&self) -> Result<UapiRequest> {
        let consumer = if self.consumer.is_empty() {
            default_consumer().as_str().into()
        } else {
            self.consumer.as_str().into()
        };
        Ok(UapiRequest::Line(v2::LineRequest {
            offsets: v2::Offsets::from_slice(&self.cfg.offsets),
            consumer,
            event_buffer_size: self.kernel_event_buffer_size,
            num_lines: self.cfg.offsets.len() as u32,
            config: self.cfg.to_v2()?,
            ..Default::default()
        }))
    }
}

#[cfg_attr(test, derive(Debug))]
#[allow(clippy::large_enum_variant)]
enum UapiRequest {
    #[cfg(feature = "uapi_v1")]
    Handle(v1::HandleRequest),

    #[cfg(feature = "uapi_v1")]
    Event(v1::EventRequest),

    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    Line(v2::LineRequest),
}

fn default_consumer() -> String {
    format!("gpiocdev-p{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use Bias::*;
    use Direction::*;
    use Drive::*;
    use EdgeDetection::*;
    use EventClock::*;
    use Value::*;

    #[test]
    fn default() {
        let b = Builder::default();
        assert_eq!(b.cfg.chip.as_os_str(), "");
        assert_eq!(b.cfg.num_lines(), 0);
        assert_eq!(b.consumer.as_str(), "");
        assert_eq!(b.kernel_event_buffer_size, 0);
        assert_eq!(b.user_event_buffer_size, 0);
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        assert_eq!(b.abiv, None);
    }

    #[test]
    fn from_config() {
        let mut cfg = Config::default();
        cfg.as_input().with_lines(&[1, 7, 4]);
        let b = Builder::from_config(cfg);
        assert_eq!(b.cfg.offsets, &[1, 7, 4]);
        assert_eq!(b.cfg.num_lines(), 3);
    }

    #[test]
    fn request_no_chip() {
        let res = Builder::default().with_line(2).request();
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "No chip specified.");
    }

    #[test]
    fn request_multiple_chips() {
        let res = Builder::default()
            .on_chip("/dev/gpiochip0")
            .with_line(2)
            .on_chip("/dev/gpiochip1")
            .with_line(3)
            .request();
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "Multiple chips requested.");
    }

    #[test]
    fn with_config() {
        let mut b = Builder::default();
        let mut cfg = Config::default();
        cfg.as_input().with_lines(&[1, 7, 4]);
        b.with_config(cfg);
        assert_eq!(b.cfg.offsets, &[1, 7, 4]);
        assert_eq!(b.cfg.num_lines(), 3);
    }

    #[test]
    fn config() {
        let mut b = Builder::default();
        b.as_input().with_lines(&[1, 7, 4]);
        let cfg = b.config();
        assert_eq!(cfg.num_lines(), 3);
        assert_eq!(cfg.offsets, &[1, 7, 4]);
    }

    #[test]
    fn with_consumer() {
        let mut b = Builder::default();
        assert_eq!(b.consumer.as_str(), "");

        b.with_consumer("builder test");
        assert_eq!(b.consumer.as_str(), "builder test");
    }

    #[test]
    fn with_kernel_event_buffer_size() {
        let mut b = Builder::default();
        assert_eq!(b.kernel_event_buffer_size, 0);

        b.with_kernel_event_buffer_size(42);
        assert_eq!(b.kernel_event_buffer_size, 42);
    }
    #[test]
    fn with_user_event_buffer_size() {
        let mut b = Builder::default();
        assert_eq!(b.user_event_buffer_size, 0);

        b.with_user_event_buffer_size(67);
        assert_eq!(b.user_event_buffer_size, 67);
    }

    #[test]
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn using_abi_version() {
        let mut b = Builder::default();
        assert_eq!(b.abiv, None);

        b.using_abi_version(AbiVersion::V1);
        assert_eq!(b.abiv, Some(AbiVersion::V1));

        b.using_abi_version(AbiVersion::V2);
        assert_eq!(b.abiv, Some(AbiVersion::V2));
    }

    #[test]
    fn on_chip() {
        let mut b = Builder::default();
        assert_eq!(b.cfg.chip.as_os_str(), "");

        b.on_chip("test chip");
        assert_eq!(b.cfg.chip.as_os_str(), "test chip");

        b.on_chip("test chip2");
        assert_eq!(b.cfg.chip.as_os_str(), "test chip");
    }

    #[test]
    fn as_input() {
        let mut b = Builder::default();
        b.as_output(Active);
        assert_eq!(b.cfg.base.direction, Some(Output));

        b.as_input();
        assert_eq!(b.cfg.base.direction, Some(Input));
    }

    #[test]
    fn as_output() {
        let mut b = Builder::default();
        b.as_input();
        assert_eq!(b.cfg.base.direction, Some(Input));

        b.as_output(Active);
        assert_eq!(b.cfg.base.direction, Some(Output));
    }

    #[test]
    fn as_active_low() {
        let mut b = Builder::default();
        assert!(!b.cfg.base.active_low);

        b.as_active_low();
        assert!(b.cfg.base.active_low);
    }

    #[test]
    fn as_active_high() {
        let mut b = Builder::default();
        assert!(!b.cfg.base.active_low);

        b.as_active_low();
        assert!(b.cfg.base.active_low);

        b.as_active_high();
        assert!(!b.cfg.base.active_low);
    }

    #[test]
    fn with_bias() {
        let mut b = Builder::default();
        b.with_bias(PullUp);
        assert_eq!(b.cfg.base.bias, Some(PullUp));

        b.with_bias(PullDown);
        assert_eq!(b.cfg.base.bias, Some(PullDown));

        b.with_bias(Disabled);
        assert_eq!(b.cfg.base.bias, Some(Disabled));

        b.with_bias(None);
        assert_eq!(b.cfg.base.bias, None);
    }

    #[test]
    fn with_debounce_period() {
        let d_us = Duration::from_micros(1234);
        let d_ns = Duration::from_nanos(234);
        let mut b = Builder::default();

        b.with_drive(OpenSource);
        assert_eq!(b.cfg.base.direction, Some(Output));

        b.with_debounce_period(Duration::from_micros(1234));
        assert_eq!(b.cfg.base.debounce_period, Some(d_us));
        assert_eq!(b.cfg.base.direction, Some(Input));

        b.with_debounce_period(Duration::from_nanos(234));
        assert_eq!(b.cfg.base.debounce_period, Some(d_ns));

        b.with_debounce_period(Duration::ZERO);
        assert!(b.cfg.base.debounce_period.is_none());
    }

    #[test]
    fn with_direction() {
        let mut b = Builder::default();
        b.with_direction(Output);
        assert_eq!(b.cfg.base.direction, Some(Output));

        b.with_direction(Input);
        assert_eq!(b.cfg.base.direction, Some(Input));
    }

    #[test]
    fn with_drive() {
        let mut b = Builder::default();
        b.with_bias(PullUp)
            .with_debounce_period(Duration::from_millis(10))
            .with_edge_detection(RisingEdge);
        assert_eq!(b.cfg.base.direction, Some(Input));
        assert_eq!(b.cfg.base.bias, Some(PullUp));
        assert_eq!(b.cfg.base.debounce_period, Some(Duration::from_millis(10)));
        assert_eq!(b.cfg.base.edge_detection, Some(RisingEdge));

        b.with_drive(PushPull);
        assert_eq!(b.cfg.base.direction, Some(Output));
        assert_eq!(b.cfg.base.drive, Some(PushPull));
        assert_eq!(b.cfg.base.bias, Some(PullUp));
        assert!(b.cfg.base.debounce_period.is_none());
        assert_eq!(b.cfg.base.edge_detection, None);

        b.with_drive(OpenDrain);
        assert_eq!(b.cfg.base.drive, Some(OpenDrain));

        b.with_drive(OpenSource);
        assert_eq!(b.cfg.base.drive, Some(OpenSource));
    }

    #[test]
    fn with_edge_detection() {
        let mut b = Builder::default();
        b.with_drive(OpenSource);
        assert_eq!(b.cfg.base.direction, Some(Output));

        b.with_edge_detection(RisingEdge);
        assert_eq!(b.cfg.base.edge_detection, Some(RisingEdge));
        assert_eq!(b.cfg.base.drive, None);
        assert_eq!(b.cfg.base.direction, Some(Input));

        b.with_edge_detection(FallingEdge);
        assert_eq!(b.cfg.base.edge_detection, Some(FallingEdge));

        b.with_edge_detection(BothEdges);
        assert_eq!(b.cfg.base.edge_detection, Some(BothEdges));

        b.with_edge_detection(None);
        assert_eq!(b.cfg.base.edge_detection, None);
    }

    #[test]
    fn with_event_clock() {
        let mut b = Builder::default();
        assert_eq!(b.cfg.base.event_clock, None);

        b.with_event_clock(Realtime);
        assert_eq!(b.cfg.base.event_clock, Some(Realtime));

        b.with_event_clock(Monotonic);
        assert_eq!(b.cfg.base.event_clock, Some(Monotonic));

        b.with_event_clock(Hte);
        assert_eq!(b.cfg.base.event_clock, Some(Hte));

        b.with_event_clock(None);
        assert_eq!(b.cfg.base.event_clock, None);
    }

    #[test]
    fn with_line() {
        let mut b = Builder::default();
        assert_eq!(b.cfg.num_lines(), 0);

        b.with_line(3);
        assert_eq!(b.cfg.num_lines(), 1);

        b.with_line(5);
        assert_eq!(b.cfg.num_lines(), 2);

        b.with_line(3);
        assert_eq!(b.cfg.num_lines(), 2);
    }

    #[test]
    fn without_line() {
        let mut b = Builder::default();

        b.with_lines(&[3, 1]);
        assert_eq!(b.cfg.num_lines(), 2);

        b.without_line(3);
        assert_eq!(b.cfg.num_lines(), 1);
        assert_eq!(b.cfg.offsets, &[1]);
    }

    #[test]
    fn with_lines() {
        let mut b = Builder::default();
        assert_eq!(b.cfg.num_lines(), 0);

        b.with_lines(&[3, 1]);
        assert_eq!(b.cfg.num_lines(), 2);
        assert_eq!(b.cfg.offsets, &[3, 1]);

        b.with_lines(&[1, 5]);
        assert_eq!(b.cfg.num_lines(), 3);
        assert_eq!(b.cfg.offsets, &[3, 1, 5]);
    }

    #[test]
    fn without_lines() {
        let mut b = Builder::default();
        b.with_lines(&[3, 1, 0, 5, 7]);
        assert_eq!(b.cfg.num_lines(), 5);

        b.without_lines(&[3, 5]);
        assert_eq!(b.cfg.num_lines(), 3);
        assert_eq!(b.cfg.offsets, &[1, 0, 7]);
    }

    fn sorted(s: &[u32]) -> Vec<u32> {
        let mut x = s.to_vec();
        x.sort();
        x
    }

    #[test]
    fn with_output_lines() {
        let mut b = Builder::default();
        assert_eq!(b.cfg.num_lines(), 0);

        let vv: Values = [(3, Value::Active), (5, Value::Inactive)]
            .into_iter()
            .collect();

        b.with_output_lines(&vv);
        assert_eq!(b.cfg.num_lines(), 2);
        assert_eq!(sorted(&b.cfg.offsets), &[3, 5]);
        assert!(b.cfg.lcfg.contains_key(&3));
        assert_eq!(b.cfg.lcfg.get(&3).unwrap().direction, Some(Output));
        assert_eq!(b.cfg.lcfg.get(&3).unwrap().value, Some(Value::Active));
        assert!(b.cfg.lcfg.contains_key(&5));
        assert_eq!(b.cfg.lcfg.get(&5).unwrap().direction, Some(Output));
        assert_eq!(b.cfg.lcfg.get(&5).unwrap().value, Some(Value::Inactive));

        let vv: Values = [(3, Value::Inactive), (1, Value::Active)]
            .into_iter()
            .collect();

        b.with_output_lines(&vv);
        assert_eq!(b.cfg.num_lines(), 3);
        assert_eq!(sorted(&b.cfg.offsets), &[1, 3, 5]);
        assert!(b.cfg.lcfg.contains_key(&3));
        assert_eq!(b.cfg.lcfg.get(&3).unwrap().direction, Some(Output));
        assert_eq!(b.cfg.lcfg.get(&3).unwrap().value, Some(Value::Inactive));
        assert!(b.cfg.lcfg.contains_key(&1));
        assert_eq!(b.cfg.lcfg.get(&1).unwrap().direction, Some(Output));
        assert_eq!(b.cfg.lcfg.get(&1).unwrap().value, Some(Value::Active));
    }

    #[test]
    fn with_value() {
        let mut b = Builder::default();
        assert_eq!(b.cfg.base.value, None);

        b.as_input().with_value(Active);
        assert_eq!(b.cfg.base.value, Some(Active));

        b.as_input().with_value(Inactive);
        assert_eq!(b.cfg.base.value, Some(Inactive));
    }

    #[test]
    fn from_line_config() {
        let mut b = Builder::default();
        let d_us = Duration::from_micros(1234);
        let lc = line::Config {
            direction: Some(Output),
            active_low: true,
            bias: Some(Disabled),
            drive: Some(OpenSource),
            edge_detection: Some(BothEdges),
            event_clock: Some(Realtime),
            debounce_period: Some(d_us),
            value: Some(Active),
        };
        b.from_line_config(&lc);
        assert!(b.cfg.lcfg.is_empty());
        assert!(b.cfg.selected.is_empty());
        let base = b.cfg.base;
        assert_eq!(base.direction, Some(Output));
        assert!(base.active_low);
        assert_eq!(base.direction, Some(Output));
        assert_eq!(base.drive, Some(OpenSource));
        assert_eq!(base.edge_detection, Some(BothEdges));
        assert_eq!(base.event_clock, Some(Realtime));
        assert_eq!(base.value, Some(Active));
        assert_eq!(base.debounce_period, Some(d_us));
    }

    #[test]
    fn to_uapi() {
        let res = Builder::default().to_uapi();
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "No lines specified.");
    }

    #[test]
    #[cfg(feature = "uapi_v1")]
    fn to_v1() {
        let mut b = Builder::default();
        b.with_consumer("test builder")
            .with_lines(&[1, 8, 4])
            .as_active_low()
            .as_input();
        if let UapiRequest::Handle(hr) = b.to_v1().unwrap() {
            assert_eq!(hr.num_lines, 3);
            assert_eq!(hr.offsets.get(0), 1);
            assert_eq!(hr.offsets.get(1), 8);
            assert_eq!(hr.offsets.get(2), 4);
            assert_eq!(String::from(&hr.consumer).as_str(), "test builder");
            assert!(hr
                .flags
                .contains(v1::HandleRequestFlags::INPUT | v1::HandleRequestFlags::ACTIVE_LOW));
            assert_eq!(hr.values.get(0), 0);
            assert_eq!(hr.values.get(1), 0);
            assert_eq!(hr.values.get(2), 0);
        } else {
            panic!("not a handle request");
        }

        let mut b = Builder::default();
        b.with_consumer("test builder")
            .with_line(8)
            .with_edge_detection(RisingEdge);
        if let UapiRequest::Event(er) = b.to_v1().unwrap() {
            assert_eq!(er.offset, 8);
            assert_eq!(String::from(&er.consumer).as_str(), "test builder");
            assert!(er.handleflags.contains(v1::HandleRequestFlags::INPUT));
            assert!(er.eventflags.contains(v1::EventRequestFlags::RISING_EDGE));
        } else {
            panic!("not an event request");
        }

        let mut b = Builder::default();
        b.with_lines(&[2, 6])
            .as_output(Value::Active)
            .with_lines(&[5, 7])
            .as_output(Value::Inactive);
        if let UapiRequest::Handle(hr) = b.to_v1().unwrap() {
            assert_eq!(hr.num_lines, 4);
            assert_eq!(hr.offsets.get(0), 2);
            assert_eq!(hr.offsets.get(1), 6);
            assert_eq!(hr.offsets.get(2), 5);
            assert_eq!(hr.offsets.get(3), 7);
            assert_eq!(
                String::from(&hr.consumer).as_str(),
                format!("gpiocdev-p{}", std::process::id()).as_str()
            );
            assert!(hr.flags.contains(v1::HandleRequestFlags::OUTPUT));
            assert_eq!(hr.values.get(0), 1);
            assert_eq!(hr.values.get(1), 1);
            assert_eq!(hr.values.get(2), 0);
            assert_eq!(hr.values.get(3), 0);
        } else {
            panic!("not a handle request");
        }

        // ABI limitations

        let mut b = Builder::default();
        assert_eq!(
            b.with_kernel_event_buffer_size(42)
                .with_lines(&[1, 8, 4])
                .as_input()
                .to_v1()
                .unwrap_err()
                .to_string(),
            "uAPI ABI v1 does not support setting event buffer size."
        );

        let mut b = Builder::default();
        assert_eq!(
            b.with_lines(&[1, 8, 4])
                .as_input()
                .with_line(3)
                .as_output(Value::Active)
                .to_v1()
                .unwrap_err()
                .to_string(),
            "uAPI ABI v1 requires all lines to share the same configuration."
        );

        let mut b = Builder::default();
        assert_eq!(
            b.with_lines(&[1, 8, 4])
                .with_edge_detection(RisingEdge)
                .to_v1()
                .unwrap_err()
                .to_string(),
            "uAPI ABI v1 only supports edge detection on single line requests."
        );

        let mut b = Builder::default();
        assert_eq!(
            b.with_lines(&[1, 8, 4])
                .with_debounce_period(Duration::from_millis(23))
                .to_v1()
                .unwrap_err()
                .to_string(),
            "uAPI ABI v1 does not support debounce."
        );

        let mut b = Builder::default();
        assert_eq!(
            b.with_line(4)
                .with_edge_detection(RisingEdge)
                .with_event_clock(EventClock::Realtime)
                .to_v1()
                .unwrap_err()
                .to_string(),
            "uAPI ABI v1 does not support selecting the event clock source."
        );
    }

    #[test]
    #[allow(irrefutable_let_patterns)]
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    fn to_v2() {
        let mut b = Builder::default();
        b.with_consumer("test builder")
            .with_kernel_event_buffer_size(42)
            .with_lines(&[1, 8, 4])
            .with_edge_detection(RisingEdge)
            .with_event_clock(EventClock::Realtime);
        if let UapiRequest::Line(lr) = b.to_v2().unwrap() {
            assert_eq!(lr.num_lines, 3);
            assert_eq!(lr.offsets.get(0), 1);
            assert_eq!(lr.offsets.get(1), 8);
            assert_eq!(lr.offsets.get(2), 4);
            assert_eq!(lr.event_buffer_size, 42);
            assert_eq!(String::from(&lr.consumer).as_str(), "test builder");
            assert!(lr.config.flags.contains(
                v2::LineFlags::INPUT
                    | v2::LineFlags::EDGE_RISING
                    | v2::LineFlags::EVENT_CLOCK_REALTIME
            ));
            assert_eq!(lr.config.num_attrs, 0);
        } else {
            panic!("not a line request");
        }

        let mut b = Builder::default();
        b.with_lines(&[1, 8, 4])
            .as_input()
            .as_active_low()
            .with_lines(&[2, 6])
            .as_output(Value::Active)
            .with_lines(&[5, 7])
            .as_output(Value::Inactive);
        if let UapiRequest::Line(lr) = b.to_v2().unwrap() {
            assert_eq!(lr.num_lines, 7);
            assert_eq!(lr.offsets.get(0), 1);
            assert_eq!(lr.offsets.get(1), 8);
            assert_eq!(lr.offsets.get(2), 4);
            assert_eq!(lr.offsets.get(3), 2);
            assert_eq!(lr.offsets.get(4), 6);
            assert_eq!(lr.offsets.get(5), 5);
            assert_eq!(lr.offsets.get(6), 7);
            assert_eq!(lr.event_buffer_size, 0);
            assert_eq!(
                String::from(&lr.consumer).as_str(),
                format!("gpiocdev-p{}", std::process::id()).as_str()
            );
            // default flags match most lines, so output
            assert!(lr.config.flags.contains(v2::LineFlags::OUTPUT));
            assert_eq!(lr.config.num_attrs, 2);
            // first is flags for line inputs
            let lca = lr.config.attrs.0[0];
            assert_eq!(lca.mask, 0b000111);
            assert_eq!(lca.attr.kind, v2::LineAttributeKind::Flags);
            assert_eq!(
                lca.attr.to_value().unwrap(),
                v2::LineAttributeValue::Flags(v2::LineFlags::INPUT | v2::LineFlags::ACTIVE_LOW)
            );
            // second is values for outputs
            let lca = lr.config.attrs.0[1];
            assert_eq!(lca.mask, 0b1111000);
            assert_eq!(lca.attr.kind, v2::LineAttributeKind::Values);
            // inputs should be inactive, outputs as per config
            assert_eq!(
                lca.attr.to_value().unwrap(),
                v2::LineAttributeValue::Values(0b0011000)
            );
        } else {
            panic!("not a line request");
        }
    }
}
