// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::chip::Chip;
use super::line::{
    self, Bias, Direction, Drive, EdgeDetection, EdgeEvent, EventClock, Offset, Offsets, Value,
    Values,
};
use crate::{AbiVersion, Error, Result, UapiCall};
#[cfg(feature = "uapi_v2")]
use bitmaps::Bitmap;
#[cfg(not(feature = "uapi_v2"))]
use gpiocdev_uapi::v1 as uapi;
#[cfg(feature = "uapi_v1")]
use gpiocdev_uapi::v1;
use gpiocdev_uapi::NUM_LINES_MAX;
#[cfg(feature = "uapi_v2")]
use gpiocdev_uapi::{v2, v2 as uapi};
use nohash_hasher::IntMap;
use std::cmp::max;
#[cfg(feature = "uapi_v2")]
use std::collections::HashMap;
use std::fs::File;
use std::mem;
use std::os::unix::prelude::{AsRawFd, RawFd};
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
/// # use gpiocdev::Result;
/// # use gpiocdev::request::Request;
/// # fn main() -> Result<()> {
/// let l3 = Request::builder()
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
/// # use gpiocdev::Result;
/// # use gpiocdev::request::Request;
/// # use gpiocdev::line::Values;
/// # fn main() -> Result<()> {
/// let offsets = &[3,5];
/// let req = Request::builder()
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
/// # use gpiocdev::Result;
/// # use gpiocdev::request::{Config, Request};
/// # use gpiocdev::line::Value;
/// # fn main() -> Result<()> {
/// let mut cfg = Config::default();
/// cfg.with_line(5).as_output(Value::Active);
/// let req = Request::from_config(cfg)
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
    chip: PathBuf,
    cfg: Config,
    consumer: String,
    kernel_event_buffer_size: u32,
    user_event_buffer_size: usize,
    err: Option<Error>,
    /// The ABI version used to create the request, and so determines how to decode events.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    abiv: AbiVersion,
}

impl Builder {
    /// Start building a new request using the provided config.
    fn from_config(cfg: Config) -> Self {
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
    ///
    /// This is the terminal operation for the `Builder`.
    pub fn request(&self) -> Result<Request> {
        if let Some(e) = &self.err {
            return Err(e.clone());
        }
        if self.chip.as_os_str().is_empty() {
            return Err(Error::InvalidArgument("No chip specified.".to_string()));
        }
        let chip = Chip::from_path(&self.chip)?;
        self.do_request(&chip).map(|f| self.to_request(f))
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_request(&self, chip: &Chip) -> Result<File> {
        match self.to_uapi()? {
            UapiRequest::Handle(hr) => v1::get_line_handle(chip.fd, hr)
                .map_err(|e| Error::UapiError(UapiCall::GetLineHandle, e)),
            UapiRequest::Event(er) => v1::get_line_event(chip.fd, er)
                .map_err(|e| Error::UapiError(UapiCall::GetLineEvent, e)),
            UapiRequest::Line(lr) => {
                v2::get_line(chip.fd, lr).map_err(|e| Error::UapiError(UapiCall::GetLine, e))
            }
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_request(&self, chip: &Chip) -> Result<File> {
        match self.to_uapi()? {
            UapiRequest::Handle(hr) => v1::get_line_handle(chip.fd, hr)
                .map_err(|e| Error::UapiError(UapiCall::GetLineHandle, e)),
            UapiRequest::Event(er) => v1::get_line_event(chip.fd, er)
                .map_err(|e| Error::UapiError(UapiCall::GetLineEvent, e)),
        }
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_request(&self, chip: &Chip) -> Result<File> {
        match self.to_uapi()? {
            UapiRequest::Line(lr) => {
                v2::get_line(chip.fd, lr).map_err(|e| Error::UapiError(UapiCall::GetLine, e))
            }
        }
    }

    fn to_request(&self, f: File) -> Request {
        let fd = f.as_raw_fd();
        Request {
            _f: f,
            fd,
            offsets: self.cfg.offsets.clone(),
            cfg: Arc::new(RwLock::new(self.cfg.clone())),
            user_event_buffer_size: max(self.user_event_buffer_size, 1),
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            abiv: self.abiv,
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
    /// # use gpiocdev::Result;
    /// # use gpiocdev::request::Request;
    /// # use gpiocdev::line::Values;
    /// # fn main() -> Result<()> {
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
    /// The version can be tested for availabilty using [`supports_abi_version`].
    ///
    /// [`supports_abi_version`]: fn@crate::supports_abi_version
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    pub fn using_abi_version(&mut self, abiv: AbiVersion) -> &mut Self {
        self.abiv = abiv;
        self
    }

    /// Set the chip from which to request lines.
    ///
    /// This applies to all lines in the request. It is not possible to request lines
    /// from different chips in the same request.
    ///
    /// The request locks to the first chip provided to it by [`on_chip`] or [`with_found_line`]
    /// and ignores any subsequent changes to chip.
    ///
    /// The chip is identified by a path which must resolve to a GPIO character device.
    ///
    /// [`on_chip`]: #method.on_chip
    /// [`with_found_line`]: #method.with_found_line
    pub fn on_chip<P: Into<PathBuf>>(&mut self, path: P) -> &mut Self {
        if self.chip.as_os_str().is_empty() {
            self.chip = path.into();
        } else if self.chip != path.into() {
            self.err = Some(Error::InvalidArgument(
                "Multiple chips requested.".to_string(),
            ))
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
    pub fn with_event_clock(&mut self, event_clock: EventClock) -> &mut Self {
        self.cfg.with_event_clock(event_clock);
        self
    }

    /// Add a found line to the request.
    ///
    /// The line must be on the same chip as any existing lines in the request, or the line is ignored.
    ///
    /// Note that all configuration mutators applied subsequently only apply to this line.
    pub fn with_found_line(&mut self, line: &super::FoundLine) -> &mut Self {
        self.on_chip(&line.chip);
        if self.chip == line.chip {
            self.cfg.with_line(line.info.offset);
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

    /// Remove a lines from the request.
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
            return Err(Error::InvalidArgument("No lines specified.".to_string()));
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
        match self.abiv {
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
                "does not support setting event buffer size".to_string(),
            ));
        }
        let lcfg = self.cfg.unique()?;
        if lcfg.debounce_period.is_some() {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "does not support debounce".to_string(),
            ));
        }
        if lcfg.event_clock.is_some() {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "does not support selecting the event clock source".to_string(),
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
                    "only supports edge detection on single line requests".to_string(),
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

fn default_consumer() -> String {
    format!("gpiocdev-p{}", std::process::id())
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

/// The configuration for a request for one or more lines.
///
/// The configuration for a subset of lines is updated by selecting the lines and then calling
/// the appropriate mutators. If no lines are selected then the mutators modify the base configuration
/// that lines inherit when they are first added.
///
/// # Examples
/// ```
///    use gpiocdev::line::{Bias::*, Value::*};
///    use gpiocdev::request::Config;
///
///    let cfg = Config::default()
///        .as_input()
///        .with_bias(PullUp)
///        // -- base config ends here - just before lines are added.
///        .with_lines(&[3, 5, 8]) // lines 3,5,8 will be input with pull-up bias...
///        // -- config added here would apply to lines 3,5 and 8
///        .with_line(3) // make line 3 pull-down instead...
///        .with_bias(PullDown)
///        .with_line(4) // and line 4 an output set inactive (and pull-up from the base)
///        .as_output(Inactive);
///
/// ```
///
/// Note that the configuration is applied to hardware via a call to [`Builder.request`] or
/// [`Request.reconfigure`].  Changes to the `Config` object, either before or after that
/// only update the configuration in memory in preparation for the next application.
///
/// [`Builder.request`]: struct.Builder.html#method.request
/// [`Request.reconfigure`]: struct.Request.html#method.reconfigure
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Config {
    /// The base configuration that applies to a line when it is first added.
    ///
    /// Prior to adding lines this config is the receiver for all mutations.
    base: line::Config,

    /// The configuration for the lines.
    lcfg: IntMap<Offset, line::Config>,

    /// The set of lines described by this configuration, in order added.
    offsets: Vec<Offset>,

    /// The current subset of lines being configured.
    ///
    /// If empty then the base config is selected.
    selected: Vec<Offset>,
}

impl Config {
    /// Update the line configs using the updated config.
    ///
    /// Other fields are not updated as they are effectively immutable for the
    /// lifetime of the associated request.
    fn update(&mut self, cfg: Config) {
        self.lcfg = cfg.lcfg;
    }

    /// Set the selected lines to input.
    ///
    /// This is a short form of [`with_direction(Input)`].
    ///
    /// This is the default direction setting.
    ///
    /// [`with_direction(Input)`]: #method.with_direction
    pub fn as_input(&mut self) -> &mut Self {
        for cfg in self.selected_iter() {
            cfg.direction = Some(Direction::Input);
            // set output specific options back to default
            cfg.drive = None;
            cfg.value = None;
        }
        self
    }

    /// Do not set the direction of the selected lines.
    pub fn as_is(&mut self) -> &mut Self {
        for cfg in self.selected_iter() {
            cfg.direction = None;
            // set input/output specific options back to default
            cfg.drive = None;
            cfg.value = None;
            cfg.edge_detection = None;
            cfg.debounce_period = None;
        }
        self
    }

    /// Set the selected lines to output with the given value.
    ///
    /// This is a long form of [`with_direction(Output)`] that allows the
    /// value to be set in the same call.
    ///
    /// [`with_direction(Output)`]: #method.with_direction
    pub fn as_output(&mut self, value: Value) -> &mut Self {
        for cfg in self.selected_iter() {
            cfg.direction = Some(Direction::Output);
            cfg.value = Some(value);
            // set input specific options back to default
            cfg.edge_detection = None;
            cfg.debounce_period = None;
        }
        self
    }

    /// Set the selected lines to active low.
    pub fn as_active_low(&mut self) -> &mut Self {
        for cfg in self.selected_iter() {
            cfg.active_low = true;
        }
        self
    }

    /// Set the selected lines to active high.
    ///
    /// This is the default active level setting.
    pub fn as_active_high(&mut self) -> &mut Self {
        for cfg in self.selected_iter() {
            cfg.active_low = false;
        }
        self
    }

    /// Set the bias setting for the selected lines.
    pub fn with_bias<B: Into<Option<Bias>>>(&mut self, bias: B) -> &mut Self {
        let bias = bias.into();
        for cfg in self.selected_iter() {
            cfg.bias = bias;
        }
        self
    }

    /// Set the debounce period for the selected lines.
    ///
    /// Implicitly selects the lines as inputs, if they weren't already, and removes
    /// any output specific settings.
    pub fn with_debounce_period(&mut self, period: Duration) -> &mut Self {
        let dp = if period.is_zero() { None } else { Some(period) };
        for cfg in self.selected_iter() {
            cfg.debounce_period = dp;
            cfg.direction = Some(Direction::Input);
            cfg.drive = None;
            cfg.value = None;
        }
        self
    }

    /// Set the direction of the selected lines.
    ///
    /// Setting to input removes any output specific settings.
    ///
    /// Setting to output removes any input specific settings.
    ///
    /// Note that selecting a line as output will default its value to inactive.
    /// To provide a value use [`with_value`], or use [`as_output(value)`] instead.
    ///
    /// To determine the state of en existing output line, first request it [`as_is`],
    /// then reconfigure it as an output with an appropriate value.
    ///
    /// [`with_value`]: #method.with_value
    /// [`as_output(Value)`]: #method.as_output
    /// [`as_is`]: #method.as_is
    pub fn with_direction(&mut self, direction: Direction) -> &mut Self {
        for cfg in self.selected_iter() {
            cfg.direction = Some(direction);
            match direction {
                Direction::Output => {
                    // set input specific options back to default
                    cfg.edge_detection = None;
                    cfg.debounce_period = None;
                }
                Direction::Input => {
                    // set output specific options back to default
                    cfg.drive = None;
                    cfg.value = None;
                }
            }
        }
        self
    }

    /// Set the drive setting for the selected lines.
    ///
    /// Implicitly sets the lines as outputs, if they weren't already, and removes any
    /// input specific settings.
    pub fn with_drive(&mut self, drive: Drive) -> &mut Self {
        for cfg in self.selected_iter() {
            cfg.drive = Some(drive);
            // driven lines imply output
            cfg.direction = Some(Direction::Output);
            // set input specific options back to default
            cfg.edge_detection = None;
            cfg.debounce_period = None;
        }
        self
    }

    /// Set the edge detection for the selected lines.
    ///
    /// Implicitly sets the lines as inputs and removes any output specific settings.
    pub fn with_edge_detection<E: Into<Option<EdgeDetection>>>(&mut self, edge: E) -> &mut Self {
        let edge = edge.into();
        for cfg in self.selected_iter() {
            cfg.edge_detection = edge;
            // edge detection implies input
            cfg.direction = Some(Direction::Input);
            // set output specific options back to default
            cfg.drive = None;
            cfg.value = None;
        }
        self
    }

    /// Set the clock source for edge events on the selected lines.
    pub fn with_event_clock(&mut self, event_clock: EventClock) -> &mut Self {
        for cfg in self.selected_iter() {
            cfg.event_clock = Some(event_clock);
            // clock setting does NOT imply anything about edge detection or direction
            // so leave other settings alone.
        }
        self
    }

    /// Add a line to the config.
    ///
    /// Note that all configuration mutators applied subsequently only
    /// apply to this line.
    pub fn with_line(&mut self, offset: Offset) -> &mut Self {
        self.selected.clear();
        self.select_line(&offset);
        self
    }

    /// Remove a lines from the config.
    pub fn without_line(&mut self, offset: Offset) -> &mut Self {
        self.remove_line(&offset);
        self
    }

    /// Add a set of lines to the config.
    ///
    /// Note that all configuration mutators applied subsequently only
    /// apply to this subset of lines.
    ///
    /// Passing empty offsets re-selects the base config for subsequent mutations.
    pub fn with_lines(&mut self, offsets: &[Offset]) -> &mut Self {
        self.selected.clear();
        for offset in offsets {
            self.select_line(offset);
        }
        self
    }

    /// Remove a set of lines from the config.
    pub fn without_lines(&mut self, offsets: &[Offset]) -> &mut Self {
        for offset in offsets {
            self.remove_line(offset);
        }
        self
    }

    /// Set the value of the selected lines.
    ///
    /// This is only relevant for output lines and is ignored for input lines.
    pub fn with_value(&mut self, value: Value) -> &mut Self {
        for cfg in self.selected_iter() {
            cfg.value = Some(value);
        }
        self
    }

    /// Apply the configuration based on the snapshot from a single line.
    pub fn from_line_config(&mut self, lc: &line::Config) -> &mut Self {
        for cfg in self.selected_iter() {
            *cfg = lc.clone();
        }
        self
    }

    fn selected_iter(&mut self) -> SelectedIterator {
        SelectedIterator {
            cfg: self,
            index: 0,
        }
    }
    /// Get the requested configuration for a particular line.
    ///
    /// This is the configuration that would be applied to the line if
    /// request were to be called.
    pub fn line_config(&self, offset: Offset) -> Option<&line::Config> {
        self.lcfg.get(&offset)
    }

    /// Returns the set of lines described by the Config.
    ///
    /// Lines are in the order first added by calls to [`with_line`] or [`with_lines`].
    ///
    /// [`with_line`]: #method.with_line
    /// [`with_lines`]: #method.with_lines
    pub fn lines(&self) -> &Offsets {
        &self.offsets
    }

    /// Returns the number of lines currently described by the Config.
    pub fn num_lines(&self) -> usize {
        self.lcfg.len()
    }

    /// Returns the config that applies to all lines, or an error if the lines have
    /// distinct configurations.
    #[cfg(feature = "uapi_v1")]
    fn unique(&self) -> Result<&line::Config> {
        // Have previously checked there is at least one line.
        // unwrap is safe here as offsets match lcfg keys
        let lcfg = self.lcfg.get(&self.offsets[0]).unwrap();
        if self.offsets.len() > 1 {
            for offset in self.offsets.iter().skip(1) {
                if !lcfg.equivalent(self.lcfg.get(offset).unwrap()) {
                    return Err(Error::AbiLimitation(
                        AbiVersion::V1,
                        "requires all lines to share the same configuration".to_string(),
                    ));
                }
            }
        }
        Ok(lcfg)
    }

    fn remove_line(&mut self, offset: &Offset) {
        self.lcfg.remove(offset);
        if let Some(index) = self.selected.iter().position(|x| *x == *offset) {
            self.selected.remove(index);
        }
        if let Some(index) = self.offsets.iter().position(|x| *x == *offset) {
            self.offsets.remove(index);
        }
    }

    fn select_line(&mut self, offset: &Offset) {
        if !self.lcfg.contains_key(offset) {
            self.lcfg.insert(*offset, self.base.clone());
        }
        if !self.selected.iter().any(|x| *x == *offset) {
            self.selected.push(*offset);
        }
        if !self.offsets.iter().any(|x| *x == *offset) {
            self.offsets.push(*offset);
        }
    }

    // Overlay one config over another.
    // Used by reconfigure to update the request config.
    // New lines cannot be added, nor can any be removed.
    // If new lines are present in top they are ignored.
    // Any lines missing from top retain their existing config.
    fn overlay(&self, top: &Config) -> Config {
        let mut cfg = Config {
            offsets: self.offsets.clone(),
            // lcfg populated below
            // other fields not required for reconfigure
            ..Default::default()
        };
        for offset in &self.offsets {
            let lc = top
                .lcfg
                .get(offset)
                .unwrap_or_else(|| self.lcfg.get(offset).unwrap());
            cfg.lcfg.insert(*offset, lc.clone());
        }
        cfg
    }

    // conversions to uapi

    // v1
    #[cfg(feature = "uapi_v1")]
    fn to_v1(&self) -> Result<v1::HandleConfig> {
        let lcfg = self.unique()?;
        Ok(v1::HandleConfig {
            flags: lcfg.into(),
            values: self.to_v1_values()?,
            ..Default::default()
        })
    }

    #[cfg(feature = "uapi_v1")]
    fn to_v1_values(&self) -> Result<v1::LineValues> {
        let mut values = v1::LineValues::default();
        for (idx, offset) in self.offsets.iter().enumerate() {
            // unwrap is safe here as offsets match lcfg keys
            values.set(idx, self.lcfg.get(offset).unwrap().value().into());
        }
        Ok(values)
    }

    // v2
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    fn to_v2(&self) -> Result<v2::LineConfig> {
        // debounced and flags provide maps from attr values to bitmap of lines using those values.
        let mut debounced: IntMap<u32, LineSet> = IntMap::default();
        let mut flags: HashMap<v2::LineFlags, LineSet> = HashMap::default();
        let mut values: v2::LineValues = Default::default();
        for (idx, offset) in self.offsets.iter().enumerate() {
            // unwrap is safe here as offsets match lcfg keys
            let lcfg = self.lcfg.get(offset).unwrap();
            let mut mask = Bitmap::default();
            mask.set(idx, true);
            let lflags: v2::LineFlags = lcfg.into();
            match flags.get_mut(&lflags) {
                Some(bits) => {
                    *bits |= mask;
                }
                None => {
                    flags.insert(lflags, mask);
                }
            };
            if let Some(dp) = lcfg.debounce_period {
                // convert to usec, adding 999ns to round up to the next microsecond.
                let dp_us = (dp + Duration::from_nanos(999)).as_micros() as u32;
                match debounced.get_mut(&dp_us) {
                    Some(bits) => {
                        *bits |= mask;
                    }
                    None => {
                        debounced.insert(dp_us, mask);
                    }
                };
            }
            if lcfg.direction == Some(Direction::Output) {
                values.mask |= mask;
                values.bits.set(idx, lcfg.value().into());
            }
        }
        // have room for 10 attributes, excluding one set of flags.
        let mut num_attrs = flags.len() + debounced.len() - 1;
        // Check bits, not mask, as kernel defaults values to 0 if not specified,
        // so all outputs set to inactive can be skipped.
        if !values.bits.is_empty() {
            num_attrs += 1;
        }
        if num_attrs > v2::NUM_ATTRS_MAX {
            return Err(Error::AbiLimitation(
                AbiVersion::V2,
                format!(
                    "supports {} attrs, configuration requires {}",
                    v2::NUM_ATTRS_MAX,
                    num_attrs
                ),
            ));
        }
        // find flags with most lines set => default flags
        let mut max_ls = 0;
        let mut base_flags = v2::LineFlags::default();
        for (flg, ls) in flags.iter() {
            if ls.len() > max_ls {
                max_ls = ls.len();
                base_flags = *flg;
            }
        }
        let mut cfg = v2::LineConfig {
            flags: base_flags,
            num_attrs: num_attrs as u32,
            ..Default::default()
        };

        // remaining flags
        num_attrs = 0;
        for (flg, mask) in flags.iter() {
            if *flg == base_flags {
                continue;
            }
            let mut attr = cfg.attr_mut(num_attrs);
            attr.mask = *mask;
            attr.attr.set_flags(*flg);
            num_attrs += 1;
        }

        // outputs values, if any
        if !values.bits.is_empty() {
            let mut attr = cfg.attr_mut(num_attrs);
            attr.mask = values.mask;
            attr.attr.set_values(values.bits);
            num_attrs += 1;
        }

        // debounced
        for (dp, mask) in debounced.iter() {
            let mut attr = cfg.attr_mut(num_attrs);
            attr.mask = *mask;
            attr.attr.set_debounce_period_us(*dp);
            num_attrs += 1;
        }

        Ok(cfg)
    }
}

#[cfg(feature = "uapi_v2")]
type LineSet = Bitmap<64>;

/// An iterator over the currently selected lines in a Config.
// This is strictly internal as external usage could invalidate the safety contract.
struct SelectedIterator<'a> {
    // the config being iterated over.
    cfg: &'a mut Config,

    // the index into the selected vector.
    index: usize,
}

impl<'a> Iterator for SelectedIterator<'a> {
    type Item = &'a mut line::Config;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cfg.selected.is_empty() {
            match self.index {
                0 => {
                    // Safety: always safe as base config guaranteed to exist.
                    let cfg_ptr: *mut line::Config = &mut self.cfg.base;
                    self.index += 1;
                    Some(unsafe { &mut *cfg_ptr })
                }
                _ => None,
            }
        } else {
            if self.index >= self.cfg.selected.len() {
                return None;
            }
            // Safety:
            // Index valid as long as selected and lcfg keys are not mutated from
            // within the iteration.
            // Else will panic.
            let cfg_ptr: *mut line::Config = &mut *self
                .cfg
                .lcfg
                .get_mut(self.cfg.selected.get(self.index).unwrap())
                .unwrap();
            self.index += 1;
            Some(unsafe { &mut *cfg_ptr })
        }
    }
}

/// An active request of a set of lines.
///
/// Requests are built by the [`Builder`], which itself can be constructed by [`builder`].
///
/// # Event Buffering
///
/// Events are buffered both in the kernel and in user space.
/// The purpose of the kernel event buffer is to store events that may occur in a burst at
/// a faster rate than they can be handled by the user space process.
/// The kernel event buffer can be customised to some extent by the [`with_kernel_event_buffer_size`] option.
///
/// A number of options are available for user space buffering,
/// i.e. where the events are to be written when they are read from the kernel.
///
/// The [`read_edge_event`] reads a single event onto the stack.
///
/// The [`EdgeEventBuffer`] provides buffering on the heap, and can reduce reading overheads
/// by reading multiple events from the kernel at once.  The [`edge_events`] iterator uses an
/// EdgeEventBuffer, the size of which is controlled by [`with_user_event_buffer_size`]
///
/// It is also possible to read multiple events into a user specified location using
/// [`read_edge_events_into_slice`].
/// As with [`EdgeEventBuffer`] this may reduce read overheads when reading a burst of events
/// from the kernel.  The [`edge_event_size`] method provides the size required to store a
/// single event to allow sizing of custom slices.
///
/// # Reading Output Values
///
/// Note that reading back output values using [`value`] or [`values`] is dependent on driver
/// and hardware support and so cannot be guaranteed to work, though frequently it does.
/// Test with your particular hardware to be sure.
///
/// [`builder`]: #method.builder
/// [`edge_events`]: #method.edge_events
/// [`edge_event_size`]: #method.edge_event_size
/// [`read_edge_event`]: #method.read_edge_event
/// [`read_edge_events_into_slice`]: #method.read_edge_events_into_slice
/// [`with_kernel_event_buffer_size`]: struct.Builder.html#method.with_kernel_event_buffer_size
/// [`with_user_event_buffer_size`]: struct.Builder.html#method.with_user_event_buffer_size
/// [`value`]: #method.value
/// [`values`]: #method.values
#[derive(Debug)]
pub struct Request {
    /// The request file.
    _f: File,

    /// Cached copy of _f.as_raw_fd() for syscalls, to avoid Arc<Mutex<>> overheads for ops.
    fd: RawFd,

    /// The offsets of the requested lines.
    offsets: Vec<Offset>,

    /// A snapshot of the active configuration for the request.
    cfg: Arc<RwLock<Config>>,

    /// The size of the user buffer created for the `edge_events` iterator.
    user_event_buffer_size: usize,

    /// The ABI version used to create the request, and so determines how to decode events.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    abiv: AbiVersion,
}

impl Request {
    /// Start building a new request.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use gpiocdev::Result;
    /// # use gpiocdev::request::Request;
    /// # use gpiocdev::line::Values;
    /// # fn main() -> Result<()> {
    /// let req = Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_lines(&[3,5])
    ///     .request()?;
    /// let mut values = Values::default();
    /// req.values(&mut values)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Start building a new request using the provided config.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use gpiocdev::Result;
    /// # use gpiocdev::line::Value;
    /// # use gpiocdev::request::{Config, Request};
    /// # fn main() -> Result<()> {
    /// let mut cfg = Config::default();
    /// cfg.with_line(5).as_output(Value::Active);
    /// let req = Request::from_config(cfg)
    ///     .on_chip("/dev/gpiochip0")
    ///     .request()?;
    /// # req.set_value(5,Value::Inactive)?;
    /// # Ok(())
    /// # }
    pub fn from_config(config: Config) -> Builder {
        Builder::from_config(config)
    }

    /// Get the values for a subset of the requested lines.
    ///
    /// The keys indicate the lines to get.
    /// Keys that are not requested offsets are ignored.
    /// If no keys are set then all requested lines are returned.
    ///
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::Result;
    /// # use gpiocdev::request::Request;
    /// # use gpiocdev::line::Values;
    /// # fn main() -> Result<()> {
    /// let req = Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_lines(&[3,5,6,8])
    ///     .request()?;
    /// // subset of lines
    /// let mut values = Values::from_offsets(&[3,8]);
    /// req.values(&mut values)?;
    /// // all requested lines
    /// let mut values = Values::default();
    /// req.values(&mut values)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn values(&self, values: &mut Values) -> Result<()> {
        self.do_values(values)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_values(&self, values: &mut Values) -> Result<()> {
        match self.abiv {
            AbiVersion::V1 => self.do_values_v1(values),
            AbiVersion::V2 => self.do_values_v2(values),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_values(&self, values: &mut Values) -> Result<()> {
        self.do_values_v1(values)
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_values(&self, values: &mut Values) -> Result<()> {
        self.do_values_v2(values)
    }
    #[cfg(feature = "uapi_v1")]
    fn do_values_v1(&self, values: &mut Values) -> Result<()> {
        let mut vals = v1::LineValues::default();
        v1::get_line_values(self.fd, &mut vals)
            .map_err(|e| Error::UapiError(UapiCall::GetLineValues, e))?;
        if values.is_empty() {
            values.overlay_from_v1(&self.offsets, &vals);
            return Ok(());
        }
        for i in 0..self.offsets.len() {
            if values.contains_key(&self.offsets[i]) {
                values.set(self.offsets[i], vals.get(i).into());
            }
        }
        Ok(())
    }
    #[cfg(feature = "uapi_v2")]
    fn do_values_v2(&self, values: &mut Values) -> Result<()> {
        let mut vals = values.to_v2(&self.offsets);
        v2::get_line_values(self.fd, &mut vals)
            .map(|_| values.overlay_from_v2(&self.offsets, &vals))
            .map_err(|e| Error::UapiError(UapiCall::GetLineValues, e))
    }

    /// Get the value for one line in the request.
    ///
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::Result;
    /// # use gpiocdev::request::Request;
    /// # use gpiocdev::line::Values;
    /// # fn main() -> Result<()> {
    /// let req = Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_lines(&[3,5])
    ///     .request()?;
    /// let v5 = req.value(5)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn value(&self, offset: Offset) -> Result<Value> {
        if !self.offsets.contains(&offset) {
            return Err(Error::InvalidArgument(
                "offset is not a requested line.".to_string(),
            ));
        }
        let mut values = Values::default();
        values.set(offset, Value::Inactive);
        self.values(&mut values)?;
        values.get(offset).ok_or_else(|| {
            Error::UnexpectedResponse("Requested value not present in response.".to_string())
        })
    }

    /// Set the values for a subset of the requested lines.
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::Result;
    /// # use gpiocdev::line::Value::{Active, Inactive};
    /// # use gpiocdev::line::Values;
    /// # use gpiocdev::request::Request;
    /// # fn main() -> Result<()> {
    /// let req = Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_lines(&[3,5,6,8])
    ///     .as_output(Active)
    ///     .request()?;
    /// let mut values = Values::default();
    /// values.set(5, Inactive).set(6, Inactive);
    /// req.set_values(&values)?;
    /// # Ok(())
    /// # }
    pub fn set_values(&self, values: &Values) -> Result<()> {
        self.do_set_values(values)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_set_values(&self, values: &Values) -> Result<()> {
        match self.abiv {
            AbiVersion::V1 => self.do_set_values_v1(values),
            AbiVersion::V2 => self.do_set_values_v2(values),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_set_values(&self, values: &Values) -> Result<()> {
        self.do_set_values_v1(values)
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_set_values(&self, values: &Values) -> Result<()> {
        self.do_set_values_v2(values)
    }
    #[cfg(feature = "uapi_v1")]
    fn do_set_values_v1(&self, values: &Values) -> Result<()> {
        if self.offsets.iter().any(|o| !values.contains_key(o)) {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "requires all requested lines".to_string(),
            ));
        }
        v1::set_line_values(self.fd, &values.to_v1(&self.offsets))
            .map_err(|e| Error::UapiError(UapiCall::SetLineValues, e))
    }
    #[cfg(feature = "uapi_v2")]
    fn do_set_values_v2(&self, values: &Values) -> Result<()> {
        let lv = &values.to_v2(&self.offsets);
        if lv.mask.is_empty() {
            return Err(Error::InvalidArgument(
                "no requested lines in set values.".to_string(),
            ));
        }
        v2::set_line_values(self.fd, lv).map_err(|e| Error::UapiError(UapiCall::SetLineValues, e))
    }

    /// Set the value for one line in the request.
    ///
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::Result;
    /// # use gpiocdev::line::Value;
    /// # use gpiocdev::request::{Config, Request};
    /// # fn main() -> Result<()> {
    /// let mut cfg = Config::default();
    /// cfg.with_line(5).as_output(Value::Active);
    /// let req = Request::from_config(cfg)
    ///     .on_chip("/dev/gpiochip0")
    ///     .request()?;
    /// req.set_value(5,Value::Inactive)?;
    /// # Ok(())
    /// # }
    pub fn set_value(&self, offset: Offset, value: Value) -> Result<()> {
        if !self.offsets.contains(&offset) {
            return Err(Error::InvalidArgument(
                "offset is not a requested line.".to_string(),
            ));
        }
        let mut values = Values::default();
        values.set(offset, value);
        self.set_values(&values)
    }

    /// Get a snapshot of the requested configuration.
    ///
    /// This is the configuration currently applied to the hardware.
    pub fn config(&self) -> Config {
        self.cfg
            .read()
            .expect("failed to acquire read lock on config")
            .clone()
    }

    /// Get a snapshot of the requested configuration for a particular line.
    ///
    /// This is the configuration currently applied to the line.
    pub fn line_config(&self, offset: Offset) -> Option<line::Config> {
        self.cfg
            .read()
            .expect("failed to acquire read lock on config")
            .line_config(offset)
            .cloned()
    }

    /// Reconfigure the request with the an updated configuration.
    ///
    /// Note that lines cannot be added or removed from the request.
    /// Any additional lines in `new_cfg` will be ignored, and any missing
    /// lines will retain their existing configuration.
    pub fn reconfigure(&self, new_cfg: &Config) -> Result<()> {
        let cfg = self
            .cfg
            .read()
            .expect("failed to acquire read lock on config")
            .overlay(new_cfg);
        self.do_reconfigure(&cfg)?;
        // only update request config if reconfigure succeeds.
        self.cfg
            .write()
            .expect("failed to acquire write lock on config")
            .update(cfg);
        Ok(())
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_reconfigure(&self, cfg: &Config) -> Result<()> {
        match self.abiv {
            AbiVersion::V1 => {
                if self
                    .cfg
                    .read()
                    .expect("failed to acquire read lock on config")
                    .unique()?
                    .edge_detection
                    .is_some()
                {
                    return Err(Error::AbiLimitation(
                        AbiVersion::V1,
                        "cannot reconfigure lines with edge detection".to_string(),
                    ));
                }
                if cfg.unique()?.edge_detection.is_some() {
                    return Err(Error::AbiLimitation(
                        AbiVersion::V1,
                        "cannot reconfigure edge detection".to_string(),
                    ));
                }
                v1::set_line_config(self.fd, cfg.to_v1()?)
                    .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e))
            }
            AbiVersion::V2 => v2::set_line_config(self.fd, cfg.to_v2()?)
                .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e)),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_reconfigure(&self, cfg: &Config) -> Result<()> {
        if self
            .cfg
            .read()
            .expect("failed to acquire read lock on config")
            .unique()?
            .edge_detection
            .is_some()
        {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "cannot reconfigure lines with edge detection".to_string(),
            ));
        }
        if cfg.unique()?.edge_detection.is_some() {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "cannot reconfigure edge detection".to_string(),
            ));
        }
        v1::set_line_config(self.fd, cfg.to_v1()?)
            .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e))
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_reconfigure(&self, cfg: &Config) -> Result<()> {
        v2::set_line_config(self.fd, cfg.to_v2()?)
            .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e))
    }

    /// An iterator for events from the request.
    ///
    /// By default the events are read from the kernel individually.
    ///
    /// The iterator can be backed by a user space buffer using the
    /// [`Builder.with_user_event_buffer_size`] option in the builder.
    ///
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::Result;
    /// # use gpiocdev::line::{EdgeDetection, Value};
    /// # use gpiocdev::request::Request;
    /// # fn main() -> Result<()> {
    /// let req = Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_consumer("watcher")
    ///     .with_line(23)
    ///     .with_edge_detection(EdgeDetection::BothEdges)
    ///     .request()?;
    ///
    /// for event in req.edge_events() {
    ///     println!("{:?}", event?);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Builder.with_user_event_buffer_size`]: struct.Builder.html#method.with_user_event_buffer_size
    pub fn edge_events(&self) -> EdgeEventBuffer {
        self.new_edge_event_buffer(self.user_event_buffer_size)
    }

    /// Returns true when the request has edge events available to read using [`read_edge_event`].
    ///
    /// [`read_edge_event`]: #method.read_edge_event
    pub fn has_edge_event(&self) -> Result<bool> {
        gpiocdev_uapi::has_event(self.fd).map_err(|e| Error::UapiError(UapiCall::HasEvent, e))
    }

    /// Wait for an edge event to be available.
    ///
    /// Returns true if [`read_edge_event`] will return an event without blocking.
    ///
    /// [`read_edge_event`]: #method.read_edge_event
    pub fn wait_edge_event(&self, timeout: Duration) -> Result<bool> {
        gpiocdev_uapi::wait_event(self.fd, timeout)
            .map_err(|e| Error::UapiError(UapiCall::WaitEvent, e))
    }

    /// Read a single edge event from the request.
    ///
    /// Will block until an edge event is available.
    ///
    /// This is a convenience function.
    /// Reading events using [`edge_events`] or a buffer created using [`new_edge_event_buffer`]
    /// may be more performant.
    ///
    /// [`edge_events`]: #method.edge_events
    /// [`new_edge_event_buffer`]: #method.new_edge_event_buffer
    pub fn read_edge_event(&self) -> Result<EdgeEvent> {
        self.do_read_edge_event()
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_read_edge_event(&self) -> Result<EdgeEvent> {
        // bbuf is statically sized to the greater of the v1/v2 size so it can be placed on the stack.
        let mut bbuf = [0; mem::size_of::<v2::LineEdgeEvent>()];
        // and dynamically sliced down to the required size, if necessary
        let buf = &mut bbuf[0..self.edge_event_size()];
        let n = self.read_edge_events_into_slice(buf)?;
        self.do_edge_event_from_slice(&buf[0..n])
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_read_edge_event(&self) -> Result<EdgeEvent> {
        let mut buf = [0; mem::size_of::<uapi::LineEdgeEvent>()];
        let n = self.read_edge_events_into_slice(&mut buf)?;
        self.do_edge_event_from_slice(&buf[0..n])
    }

    /// Create an edge event buffer.
    ///
    /// * `capacity` - The number of events that can be buffered.
    pub fn new_edge_event_buffer(&self, capacity: usize) -> EdgeEventBuffer {
        let event_size = self.edge_event_size();
        EdgeEventBuffer {
            req: self,
            event_size,
            filled: 0,
            read: 0,
            buf: vec![0; max(capacity, 1) * event_size],
        }
    }

    // External buffer/slice methods.

    /// Read edge events from the kernel into a user space `[u8]` slice.
    ///
    /// This is a helper function for the special case where the user prefers to
    /// manage the buffer containing raw events, e.g. to place it in a specific
    /// location in memory.
    ///
    /// This will read in [`edge_event_size`] sized chunks so `buf` must be at least
    /// as large as one event.
    ///
    /// This function will block if no events are available to read.
    ///
    /// * `buf` - The slice to contain the raw events.
    ///
    /// [`edge_event_size`]: #method.edge_event_size
    pub fn read_edge_events_into_slice(&self, buf: &mut [u8]) -> Result<usize> {
        gpiocdev_uapi::read_event(self.fd, buf)
            .map_err(|e| Error::UapiError(UapiCall::ReadEvent, e))
    }

    /// Read an edge event from a `[u8]` slice.
    ///
    /// This is a helper function for the special case where the user prefers to
    /// manage the buffer containing raw events, e.g. to place it in a specific
    /// location in memory.
    ///
    /// Assumes the buffer has been previously populated by a call to
    /// [`read_edge_events_into_slice`].
    ///
    /// * `buf` - The slice containing the raw event.
    ///
    /// [`read_edge_events_into_slice`]: #method.read_edge_events_into_slice
    pub fn edge_event_from_slice(&self, buf: &[u8]) -> Result<EdgeEvent> {
        self.do_edge_event_from_slice(buf)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_edge_event_from_slice(&self, buf: &[u8]) -> Result<EdgeEvent> {
        Ok(match self.abiv {
            AbiVersion::V1 => {
                let mut ee = EdgeEvent::from(
                    v1::LineEdgeEvent::from_slice(buf)
                        .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
                );
                // populate offset for v1
                ee.offset = self.offsets[0];
                ee
            }
            AbiVersion::V2 => EdgeEvent::from(
                uapi::LineEdgeEvent::from_slice(buf)
                    .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
            ),
        })
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_edge_event_from_slice(&self, buf: &[u8]) -> Result<EdgeEvent> {
        let mut ee = EdgeEvent::from(
            v1::LineEdgeEvent::from_slice(buf)
                .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
        );
        // populate offset for v1
        ee.offset = self.offsets[0]; // there can be only one
        Ok(ee)
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_edge_event_from_slice(&self, buf: &[u8]) -> Result<EdgeEvent> {
        Ok(EdgeEvent::from(
            v2::LineEdgeEvent::from_slice(buf)
                .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
        ))
    }

    /// The number of bytes required to buffer a single event read from the request.
    ///
    /// This can be used to size an external `[u8]` slice to read events into.
    /// The slice size should be a multiple of this size.
    pub fn edge_event_size(&self) -> usize {
        self.do_edge_event_size()
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_edge_event_size(&self) -> usize {
        match self.abiv {
            AbiVersion::V1 => mem::size_of::<v1::LineEdgeEvent>(),
            AbiVersion::V2 => mem::size_of::<v2::LineEdgeEvent>(),
        }
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_edge_event_size(&self) -> usize {
        mem::size_of::<uapi::LineEdgeEvent>()
    }
}
impl AsRawFd for Request {
    fn as_raw_fd(&self) -> i32 {
        self.fd
    }
}

/// A user space buffer for reading edge events in bulk from a [`Request`].
///
/// Reads edge events from the kernel in bulk, where possible, while providing them
/// serially to the caller.
pub struct EdgeEventBuffer<'a> {
    req: &'a Request,

    /// The size of an individual edge event stored in the buffer.
    event_size: usize,

    /// The number of bytes currently written into the buffer
    filled: usize,

    /// The number of bytes currently read from the buffer.
    read: usize,

    /// The buffer for uAPI edge events, sized by event size and capacity
    buf: Vec<u8>,
}

impl<'a> EdgeEventBuffer<'a> {
    /// The number of events that can be stored in the buffer.
    pub fn capacity(&self) -> usize {
        self.buf.capacity() / self.event_size
    }

    /// The number of unread events currently stored in this buffer.
    ///
    /// This does not include events which may be buffered in the kernel.
    pub fn len(&self) -> usize {
        (self.filled - self.read) / self.event_size
    }

    /// Returns true if there are no unread events in the buffer.
    ///
    /// This does not check whether events are available in the kernel buffer,
    /// only if there are events currently stored in this buffer.
    pub fn is_empty(&self) -> bool {
        self.read >= self.filled
    }

    /// Returns true when either the buffer, or the request, has edge events available to read.
    pub fn has_event(&mut self) -> Result<bool> {
        if self.read < self.filled {
            return Ok(true);
        }
        self.req.has_edge_event()
    }

    /// Returns the next event from the buffer.
    ///
    /// If the buffer is empty then events are read from the request, which may block if no
    /// events are available. To avoid blocking, check [`has_event`] first, or use
    /// [`wait_event`] with a suitable timeout.
    ///
    /// [`has_event`]: #method.has_event
    /// [`wait_event`]: #method.wait_event
    pub fn read_event(&mut self) -> Result<EdgeEvent> {
        if self.read < self.filled {
            let evt_end = self.read + self.event_size;
            let evt = &self.buf[self.read..evt_end];
            self.read = evt_end;
            return self.req.edge_event_from_slice(evt);
        }
        self.read = 0;
        self.filled = 0;
        let n = self.req.read_edge_events_into_slice(&mut self.buf)?;
        // Could turn these into run-time errors, but they should never happen
        // so make them asserts to keep it simple.
        assert!(n > 0);
        assert_eq!(n % self.event_size, 0);
        self.filled = n;
        self.read = self.event_size;
        self.req
            .edge_event_from_slice(&self.buf[0..self.event_size])
    }

    /// Wait for an edge event from the request.
    ///
    /// * `timeout` - The maximum time to wait for an event.
    pub fn wait_event(&mut self, timeout: Duration) -> Result<EdgeEvent> {
        self.req.wait_edge_event(timeout)?;
        self.read_event()
    }
}

impl<'a> Iterator for EdgeEventBuffer<'a> {
    type Item = Result<EdgeEvent>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.read_event())
    }
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

    mod builder {
        use super::*;

        #[test]
        fn default() {
            let b = Builder::default();
            assert_eq!(b.chip.as_os_str(), "");
            assert_eq!(b.cfg.num_lines(), 0);
            assert_eq!(b.consumer.as_str(), "");
            assert_eq!(b.kernel_event_buffer_size, 0);
            assert_eq!(b.user_event_buffer_size, 0);
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            assert_eq!(b.abiv, AbiVersion::V2);
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
            assert_eq!(b.abiv, AbiVersion::V2);

            b.using_abi_version(AbiVersion::V1);
            assert_eq!(b.abiv, AbiVersion::V1);

            b.using_abi_version(AbiVersion::V2);
            assert_eq!(b.abiv, AbiVersion::V2);
        }

        #[test]
        fn on_chip() {
            let mut b = Builder::default();
            assert_eq!(b.chip.as_os_str(), "");

            b.on_chip("test chip");
            assert_eq!(b.chip.as_os_str(), "test chip");
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
            assert_eq!(b.cfg.lcfg.len(), 0);
            assert_eq!(b.cfg.selected.len(), 0);
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
                assert!(lca.mask.get(0));
                assert!(lca.mask.get(1));
                assert!(lca.mask.get(2));
                assert!(!lca.mask.get(3));
                assert!(!lca.mask.get(4));
                assert!(!lca.mask.get(5));
                assert!(!lca.mask.get(6));
                assert_eq!(lca.attr.kind, v2::LineAttributeKind::Flags);
                unsafe {
                    assert!(lca
                        .attr
                        .value
                        .flags
                        .contains(v2::LineFlags::INPUT | v2::LineFlags::ACTIVE_LOW));
                }
                // second is values for outputs
                let lca = lr.config.attrs.0[1];
                assert!(!lca.mask.get(0));
                assert!(!lca.mask.get(1));
                assert!(!lca.mask.get(2));
                assert!(lca.mask.get(3));
                assert!(lca.mask.get(4));
                assert!(lca.mask.get(5));
                assert!(lca.mask.get(6));
                assert_eq!(lca.attr.kind, v2::LineAttributeKind::Values);
                unsafe {
                    assert!(!lca.attr.value.values.get(0)); // inputs should be inactive
                    assert!(!lca.attr.value.values.get(1));
                    assert!(!lca.attr.value.values.get(2));
                    assert!(lca.attr.value.values.get(3)); // two active outputs
                    assert!(lca.attr.value.values.get(4));
                    assert!(!lca.attr.value.values.get(5)); // and two inactive
                    assert!(!lca.attr.value.values.get(6));
                }
            } else {
                panic!("not a line request");
            }
        }
    }

    mod config {
        use super::*;

        #[test]
        fn default() {
            let cfg = Config::default();
            assert_eq!(cfg.lcfg.len(), 0);
            assert_eq!(cfg.selected.len(), 0);
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            assert_eq!(cfg.base.direction, None);
            assert!(!cfg.base.active_low);
            assert_eq!(cfg.base.drive, None);
            assert_eq!(cfg.base.edge_detection, None);
            assert_eq!(cfg.base.event_clock, None);
            assert_eq!(cfg.base.event_clock, None);
            assert_eq!(cfg.base.value, None);
        }

        #[test]
        fn as_input() {
            let mut cfg = Config::default();
            cfg.as_output(Active).with_drive(OpenDrain);
            assert_eq!(cfg.base.direction, Some(Output));
            assert_eq!(cfg.base.value, Some(Active));
            assert_eq!(cfg.base.drive, Some(OpenDrain));

            cfg.as_input();
            assert_eq!(cfg.base.direction, Some(Input));
            assert_eq!(cfg.base.value, None);
            assert_eq!(cfg.base.drive, None);
        }

        #[test]
        fn as_output() {
            let mut cfg = Config::default();
            cfg.as_input().with_edge_detection(RisingEdge);
            assert_eq!(cfg.base.direction, Some(Input));
            assert_eq!(cfg.base.value, None);
            assert_eq!(cfg.base.edge_detection, Some(RisingEdge));

            cfg.as_output(Active);
            assert_eq!(cfg.base.direction, Some(Output));
            assert_eq!(cfg.base.value, Some(Active));
            assert_eq!(cfg.base.edge_detection, None);
        }

        #[test]
        fn as_active_low() {
            let mut cfg = Config::default();

            cfg.as_active_low();
            assert!(cfg.base.active_low);

            cfg.as_active_high();
            assert!(!cfg.base.active_low);
        }

        #[test]
        fn as_active_high() {
            let mut cfg = Config::default();
            cfg.as_active_low().with_line(3);
            assert!(cfg.lcfg.get(&3).unwrap().active_low);
            assert!(cfg.base.active_low);

            cfg.as_active_high();
            assert!(cfg.base.active_low);
            assert!(!cfg.lcfg.get(&3).unwrap().active_low);
        }

        #[test]
        fn with_bias() {
            let mut cfg = Config::default();
            cfg.with_bias(PullUp);
            assert_eq!(cfg.base.bias, Some(PullUp));

            cfg.with_bias(PullDown);
            assert_eq!(cfg.base.bias, Some(PullDown));

            cfg.with_bias(Disabled);
            assert_eq!(cfg.base.bias, Some(Disabled));

            cfg.with_bias(None);
            assert_eq!(cfg.base.bias, None);
        }

        #[test]
        fn with_debounce_period() {
            let d_us = Duration::from_micros(1234);
            let d_ns = Duration::from_nanos(234);
            let mut cfg = Config::default();
            cfg.with_drive(OpenSource);
            assert_eq!(cfg.base.direction, Some(Output));

            cfg.with_debounce_period(Duration::from_micros(1234));
            assert_eq!(cfg.base.debounce_period, Some(d_us));
            assert_eq!(cfg.base.direction, Some(Input));

            cfg.with_debounce_period(Duration::from_nanos(234));
            assert_eq!(cfg.base.debounce_period, Some(d_ns));

            cfg.with_debounce_period(Duration::ZERO);
            assert!(cfg.base.debounce_period.is_none());
        }

        #[test]
        fn with_direction() {
            let mut cfg = Config::default();
            cfg.with_direction(Output);
            assert_eq!(cfg.base.direction, Some(Output));
            cfg.with_direction(Input);
            assert_eq!(cfg.base.direction, Some(Input));
        }

        #[test]
        fn with_drive() {
            let mut cfg = Config::default();
            cfg.with_bias(PullUp)
                .with_debounce_period(Duration::from_millis(10))
                .with_edge_detection(RisingEdge);
            assert_eq!(cfg.base.direction, Some(Input));
            assert_eq!(cfg.base.bias, Some(PullUp));
            assert_eq!(cfg.base.debounce_period, Some(Duration::from_millis(10)));
            assert_eq!(cfg.base.edge_detection, Some(RisingEdge));

            cfg.with_drive(PushPull);
            assert_eq!(cfg.base.direction, Some(Output));
            assert_eq!(cfg.base.drive, Some(PushPull));
            assert_eq!(cfg.base.bias, Some(PullUp));
            assert!(cfg.base.debounce_period.is_none());
            assert_eq!(cfg.base.edge_detection, None);

            cfg.with_drive(OpenDrain);
            assert_eq!(cfg.base.drive, Some(OpenDrain));

            cfg.with_drive(OpenSource);
            assert_eq!(cfg.base.drive, Some(OpenSource));
        }

        #[test]
        fn with_edge_detection() {
            let mut cfg = Config::default();
            cfg.with_drive(OpenSource);
            assert_eq!(cfg.base.direction, Some(Output));
            cfg.with_edge_detection(RisingEdge);
            assert_eq!(cfg.base.edge_detection, Some(RisingEdge));
            assert_eq!(cfg.base.drive, None);
            assert_eq!(cfg.base.direction, Some(Input));
            cfg.with_edge_detection(FallingEdge);
            assert_eq!(cfg.base.edge_detection, Some(FallingEdge));
            cfg.with_edge_detection(BothEdges);
            assert_eq!(cfg.base.edge_detection, Some(BothEdges));
        }

        #[test]
        fn with_event_clock() {
            let mut cfg = Config::default();
            assert_eq!(cfg.base.event_clock, None);
            cfg.with_event_clock(Realtime);
            assert_eq!(cfg.base.event_clock, Some(Realtime));
            cfg.with_event_clock(Monotonic);
            assert_eq!(cfg.base.event_clock, Some(Monotonic));
        }

        #[test]
        fn with_line() {
            let mut cfg = Config::default();

            // initial mutator hits base config
            cfg.as_output(Active);
            assert_eq!(cfg.base.direction, Some(Output));
            assert_eq!(cfg.base.value, Some(Active));
            cfg.as_input();
            assert_eq!(cfg.base.direction, Some(Input));
            assert_eq!(cfg.base.value, None);
            assert!(cfg.offsets.is_empty());
            assert!(cfg.selected.is_empty());

            // mutator hits selected config
            cfg.with_line(3);
            assert_eq!(cfg.offsets, &[3]);
            assert_eq!(cfg.selected, &[3]);
            assert_eq!(cfg.lcfg.get(&3).unwrap().direction, Some(Input));
            assert_eq!(cfg.lcfg.get(&3).unwrap().value, None);

            // mutator only hits selected config
            cfg.with_line(5);
            assert_eq!(cfg.offsets, &[3, 5]);
            assert_eq!(cfg.selected, &[5]);
            assert_eq!(cfg.lcfg.get(&5).unwrap().direction, Some(Input));
            assert_eq!(cfg.lcfg.get(&5).unwrap().value, None);
            assert_eq!(cfg.lcfg.get(&3).unwrap().direction, Some(Input));
            assert_eq!(cfg.lcfg.get(&3).unwrap().value, None);
            cfg.as_output(Inactive);
            assert_eq!(cfg.offsets, &[3, 5]);
            assert_eq!(cfg.selected, &[5]);
            assert_eq!(cfg.lcfg.get(&5).unwrap().direction, Some(Output));
            assert_eq!(cfg.lcfg.get(&5).unwrap().value, Some(Inactive));
            assert_eq!(cfg.lcfg.get(&3).unwrap().direction, Some(Input));
            assert_eq!(cfg.lcfg.get(&3).unwrap().value, None);
            assert_eq!(cfg.base.direction, Some(Input));
            assert_eq!(cfg.base.value, None);
        }

        #[test]
        fn without_line() {
            let mut cfg = Config::default();
            cfg.as_output(Active);
            assert_eq!(cfg.base.direction, Some(Output));
            assert_eq!(cfg.base.value, Some(Active));

            cfg.with_lines(&[1, 6, 2, 7]);
            assert_eq!(cfg.offsets, &[1, 6, 2, 7]);
            assert_eq!(cfg.selected, &[1, 6, 2, 7]);
            assert_eq!(cfg.lcfg.get(&2).unwrap().direction, Some(Output));
            assert_eq!(cfg.lcfg.get(&2).unwrap().value, Some(Active));
            assert_eq!(cfg.lcfg.get(&6).unwrap().direction, Some(Output));
            assert_eq!(cfg.lcfg.get(&6).unwrap().value, Some(Active));

            // from selected
            cfg.without_line(2);
            assert_eq!(cfg.offsets, &[1, 6, 7]);
            assert_eq!(cfg.selected, &[1, 6, 7]);
            assert_eq!(cfg.lcfg.get(&6).unwrap().direction, Some(Output));
            assert_eq!(cfg.lcfg.get(&6).unwrap().value, Some(Active));
            cfg.with_line(1);
            assert_eq!(cfg.offsets, &[1, 6, 7]);
            assert_eq!(cfg.selected, &[1]);
            assert_eq!(cfg.lcfg.len(), 3);
            assert_eq!(cfg.lcfg.get(&6).unwrap().direction, Some(Output));
            assert_eq!(cfg.lcfg.get(&6).unwrap().value, Some(Active));

            // from unselected
            cfg.without_line(6);
            assert_eq!(cfg.offsets, &[1, 7]);
            assert_eq!(cfg.selected, &[1]);
            assert_eq!(cfg.lcfg.len(), 2);
            assert!(cfg.lcfg.contains_key(&1));
            assert!(cfg.lcfg.contains_key(&7));
            assert!(!cfg.lcfg.contains_key(&6));

            // last select
            cfg.without_line(1);
            assert_eq!(cfg.offsets, &[7]);
            assert!(cfg.selected.is_empty());
            assert_eq!(cfg.lcfg.len(), 1);
            assert!(!cfg.lcfg.contains_key(&1));
        }

        #[test]
        fn with_lines() {
            let mut cfg = Config::default();
            cfg.as_output(Active);
            assert_eq!(cfg.base.direction, Some(Output));
            assert_eq!(cfg.base.value, Some(Active));

            // select some
            cfg.with_lines(&[7, 2, 6, 1]);
            assert_eq!(cfg.offsets, &[7, 2, 6, 1]);
            assert_eq!(cfg.selected, &[7, 2, 6, 1]);
            assert_eq!(cfg.lcfg.len(), 4);
            assert!(cfg.lcfg.contains_key(&1));
            assert!(cfg.lcfg.contains_key(&2));
            assert!(cfg.lcfg.contains_key(&6));
            assert!(cfg.lcfg.contains_key(&7));

            // add another
            cfg.with_lines(&[1, 2, 9]);
            assert_eq!(cfg.offsets, &[7, 2, 6, 1, 9]);
            assert_eq!(cfg.selected, &[1, 2, 9]);
            assert_eq!(cfg.lcfg.len(), 5);
            assert!(cfg.lcfg.contains_key(&6));
            assert!(cfg.lcfg.contains_key(&7));
            assert!(cfg.lcfg.contains_key(&1));
            assert!(cfg.lcfg.contains_key(&2));
            assert!(cfg.lcfg.contains_key(&9));

            // select none
            cfg.with_lines(&[]);
            assert_eq!(cfg.offsets, &[7, 2, 6, 1, 9]);
            assert_eq!(cfg.selected.len(), 0);
            assert_eq!(cfg.lcfg.len(), 5);
            assert!(cfg.lcfg.contains_key(&1));
            assert!(cfg.lcfg.contains_key(&2));
            assert!(cfg.lcfg.contains_key(&6));
            assert!(cfg.lcfg.contains_key(&7));
            assert!(cfg.lcfg.contains_key(&9));
        }

        #[test]
        fn without_lines() {
            let mut cfg = Config::default();
            cfg.with_lines(&[1, 2, 3, 4]).with_lines(&[5, 6, 7, 8]);

            // from selected and unselected
            cfg.without_lines(&[1, 4, 6, 7]);
            assert_eq!(cfg.selected, &[5, 8]);
            assert_eq!(cfg.lcfg.len(), 4);
            assert!(!cfg.lcfg.contains_key(&1));
            assert!(cfg.lcfg.contains_key(&2));
            assert!(cfg.lcfg.contains_key(&3));
            assert!(!cfg.lcfg.contains_key(&4));
            assert!(cfg.lcfg.contains_key(&5));
            assert!(!cfg.lcfg.contains_key(&6));
            assert!(!cfg.lcfg.contains_key(&7));
            assert!(cfg.lcfg.contains_key(&8));
        }

        #[test]
        fn with_value() {
            let mut cfg = Config::default();
            assert_eq!(cfg.base.value, None);

            cfg.as_input().with_value(Active);
            assert_eq!(cfg.base.value, Some(Active));

            cfg.as_input().with_value(Inactive);
            assert_eq!(cfg.base.value, Some(Inactive));
        }

        #[test]
        fn from_line_config() {
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

            // base
            let mut cfg = Config::default();
            cfg.from_line_config(&lc);
            assert!(cfg.lcfg.is_empty());
            assert!(cfg.selected.is_empty());
            assert_eq!(cfg.base.direction, Some(Output));
            assert!(cfg.base.active_low);
            assert_eq!(cfg.base.drive, Some(OpenSource));
            assert_eq!(cfg.base.edge_detection, Some(BothEdges));
            assert_eq!(cfg.base.event_clock, Some(Realtime));
            assert_eq!(cfg.base.value, Some(Active));
            assert_eq!(cfg.base.debounce_period, Some(d_us));

            // select
            let mut cfg = Config::default();
            cfg.with_line(1).from_line_config(&lc);
            assert_eq!(cfg.selected, &[1]);
            assert_eq!(cfg.lcfg.len(), 1);
            let select = cfg.lcfg.get(&1).unwrap();
            assert_eq!(select.direction, Some(Output));
            assert!(select.active_low);
            assert_eq!(select.direction, Some(Output));
            assert_eq!(select.drive, Some(OpenSource));
            assert_eq!(select.edge_detection, Some(BothEdges));
            assert_eq!(select.event_clock, Some(Realtime));
            assert_eq!(select.value, Some(Active));
        }

        #[test]
        fn line_config() {
            let mut cfg = Config::default();
            cfg.with_bias(PullDown);
            // no config
            assert!(cfg.line_config(2).is_none());

            // from select
            cfg.with_line(2);
            let mut lc = cfg.line_config(2);
            assert!(lc.is_some());
            assert_eq!(lc.unwrap().bias, Some(PullDown));

            // from unselected
            cfg.with_line(3);
            lc = cfg.line_config(2);
            assert!(lc.is_some());
            let lc = lc.unwrap();
            assert_eq!(lc.bias, Some(PullDown));
        }

        #[test]
        fn lines() {
            let mut cfg = Config::default();
            cfg.with_lines(&[1, 2, 4, 6]).with_lines(&[2, 6, 9]);
            // should have 1,4 unselected and 2,6,9 select
            assert_eq!(cfg.lines(), &[1, 2, 4, 6, 9]);

            cfg.without_lines(&[1, 2]);
            assert_eq!(cfg.lines(), &[4, 6, 9]);
        }

        #[test]
        fn num_lines() {
            let mut cfg = Config::default();
            cfg.with_lines(&[1, 2, 4, 6]);
            assert_eq!(cfg.num_lines(), 4);

            cfg.with_lines(&[2, 6, 9]);
            // should have 1,4 unselected and 2,6,9 select
            assert_eq!(cfg.num_lines(), 5);

            cfg.without_lines(&[1, 2]);
            assert_eq!(cfg.num_lines(), 3);
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn unique() {
            let mut cfg = Config::default();
            // multiple, but unique
            cfg.with_lines(&[1, 2, 4, 6]);
            let lc = cfg.lcfg.get(&1);
            assert_eq!(cfg.unique().unwrap(), lc.unwrap());

            // multiple, not unique
            cfg.with_lines(&[2, 6, 9]).with_bias(PullUp);
            assert_eq!(
                cfg.unique().unwrap_err().to_string(),
                "uAPI ABI v1 requires all lines to share the same configuration."
            );

            // reduce to one (line 4), so unique again
            cfg.without_lines(&[1, 2, 6, 9]);
            let lc = cfg.lcfg.get(&4);
            assert_eq!(cfg.unique().unwrap(), lc.unwrap());
        }

        #[test]
        fn overlay() {
            let mut bottom = Config::default();
            bottom.with_lines(&[1, 4, 7]).as_active_low();
            assert_eq!(bottom.num_lines(), 3);
            let mut top = Config::default();
            top.with_lines(&[3, 4]).as_active_high();
            let overlay = bottom.overlay(&top);
            assert_eq!(overlay.num_lines(), 3);

            let lc = overlay.lcfg.get(&1).unwrap();
            assert!(lc.active_low);
            assert_eq!(overlay.lcfg.get(&3), None);

            let lc = overlay.lcfg.get(&4).unwrap();
            assert!(!lc.active_low);

            let lc = overlay.lcfg.get(&7).unwrap();
            assert!(lc.active_low);
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn to_v1() {
            let mut cfg = Config::default();
            cfg.with_line(3)
                .as_output(Value::Active)
                .with_line(1)
                .as_output(Value::Inactive)
                .with_line(2)
                .as_output(Value::Active);
            let hc = cfg.to_v1().unwrap();
            assert!(hc.flags.contains(v1::HandleRequestFlags::OUTPUT));
            assert_eq!(hc.values.get(0), 1);
            assert_eq!(hc.values.get(1), 0);
            assert_eq!(hc.values.get(2), 1);

            cfg.with_line(2).as_input();
            assert_eq!(
                cfg.to_v1().unwrap_err().to_string(),
                "uAPI ABI v1 requires all lines to share the same configuration."
            );
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn to_v1_values() {
            let mut cfg = Config::default();
            cfg.with_line(3)
                .as_output(Value::Active)
                .with_line(2)
                .as_input()
                .with_line(1)
                .as_output(Value::Inactive);
            let values = cfg.to_v1_values().unwrap();
            // in order added to config
            assert_eq!(values.get(0), 1);
            assert_eq!(values.get(1), 0);
            assert_eq!(values.get(2), 0);
        }

        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn to_v2() {
            let mut cfg = Config::default();
            cfg.with_line(3)
                .as_output(Value::Active)
                .with_line(1)
                .as_output(Value::Inactive)
                .with_line(4)
                .as_input()
                .as_active_low()
                .with_debounce_period(Duration::from_millis(10))
                .with_line(2)
                .as_output(Value::Active);
            let lc = cfg.to_v2().unwrap();
            assert!(lc.flags.contains(v2::LineFlags::OUTPUT));
            assert_eq!(lc.num_attrs, 3);

            // first is flags for line 4
            let lca = lc.attrs.0[0];
            assert!(!lca.mask.get(0));
            assert!(!lca.mask.get(1));
            assert!(lca.mask.get(2));
            assert!(!lca.mask.get(3));
            assert_eq!(lca.attr.kind, v2::LineAttributeKind::Flags);
            unsafe {
                assert!(lca
                    .attr
                    .value
                    .flags
                    .contains(v2::LineFlags::INPUT | v2::LineFlags::ACTIVE_LOW));
            }

            // second is values for outputs
            let lca = lc.attrs.0[1];
            assert!(lca.mask.get(0));
            assert!(lca.mask.get(1));
            assert!(!lca.mask.get(2));
            assert!(lca.mask.get(3));
            assert_eq!(lca.attr.kind, v2::LineAttributeKind::Values);
            unsafe {
                assert!(lca.attr.value.values.get(0));
                assert!(!lca.attr.value.values.get(1));
                assert!(!lca.attr.value.values.get(2));
                assert!(lca.attr.value.values.get(3));
            }

            // third is debounce for line 4
            let lca = lc.attrs.0[2];
            assert!(!lca.mask.get(0));
            assert!(!lca.mask.get(1));
            assert!(lca.mask.get(2));
            assert!(!lca.mask.get(3));
            assert_eq!(lca.attr.kind, v2::LineAttributeKind::Debounce);
            unsafe {
                assert_eq!(lca.attr.value.debounce_period_us, 10000);
            }

            // too many attrs required
            for offset in 10..20 {
                cfg.with_line(offset)
                    .with_debounce_period(Duration::from_millis(offset as u64));
            }
            assert_eq!(
                cfg.to_v2().unwrap_err().to_string(),
                // requires 10 attrs in addition to the 3 above -
                // one for the flags and 9 for additional debounce periods.
                // (line 10 has debounce in common with line 4)
                "uAPI ABI v2 supports 10 attrs, configuration requires 13."
            );
        }
    }

    mod request {
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        use super::AbiVersion;
        use super::Request;

        #[test]
        fn request_builder() {
            let b = Request::builder();
            assert_eq!(b.chip.as_os_str(), "");
            assert_eq!(b.cfg.num_lines(), 0);
            assert_eq!(b.consumer.as_str(), "");
            assert_eq!(b.kernel_event_buffer_size, 0);
            assert_eq!(b.user_event_buffer_size, 0);
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            assert_eq!(b.abiv, AbiVersion::V2);
        }
    }
}
