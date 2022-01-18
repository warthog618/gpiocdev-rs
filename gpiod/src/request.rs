// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::chip::Chip;
use super::line::{
    self, Bias, Direction, Drive, EdgeDetection, EdgeEvent, EventClock, Offset, Offsets, Value,
    Values,
};
use super::Name;
use crate::{AbiVersion, Error, Result, UapiCall};
#[cfg(feature = "uapi_v2")]
use bitmaps::Bitmap;
#[cfg(not(feature = "uapi_v2"))]
use gpiod_uapi::v1 as uapi;
#[cfg(feature = "uapi_v1")]
use gpiod_uapi::v1;
use gpiod_uapi::NUM_LINES_MAX;
#[cfg(feature = "uapi_v2")]
use gpiod_uapi::{v2, v2 as uapi};
use nohash_hasher::IntMap;
#[cfg(feature = "uapi_v2")]
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::mem::size_of;
use std::path::PathBuf;
use std::time::Duration;

/// A builder of line requests.
///
/// Most mutators operate on the request configuration as per the [`Config`].
///
/// To request and read a basic input line:
/// ```no_run
/// # use gpiod::Result;
/// use gpiod::request::Builder;
///
/// # fn main() -> Result<()> {
/// let mut l3 = Builder::new()
///     .on_chip("/dev/gpiochip0")
///     .with_line(3)
///     .request()?;
/// let value = l3.value(3)?;
/// # Ok(())
/// # }
/// ```
///
/// Or several lines:
///
/// ```no_run
/// # use gpiod::Result;
/// use gpiod::request::Builder;
/// use gpiod::line::Values;
///
/// # fn main() -> Result<()> {
/// let offsets = &[3,5];
/// let mut ll = Builder::new()
///     .on_chip("/dev/gpiochip0")
///     .with_lines(offsets)
///     .request()?;
/// let mut values = Values::from_lines(offsets);
/// ll.values(&mut values)?;
/// # Ok(())
/// # }
/// ```
///
/// Alternatively, more complex configurations can be built separately and provided
/// to the `Builder`:
///
/// ```no_run
/// # use gpiod::Result;
/// use gpiod::request::{Config, Builder};
/// use gpiod::line::Value::*;
///
/// # fn main() -> Result<()> {
/// let mut cfg = Config::new();
/// cfg.with_line(5).as_output(Active);
/// let mut l5 = Builder::from_config(cfg)
///     .on_chip("/dev/gpiochip0")
///     .request()?;
/// l5.set_value(5,Inactive)?;
/// # Ok(())
/// # }
///
/// ```
///
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Builder {
    chip: PathBuf,
    cfg: Config,
    consumer: Name,
    event_buffer_size: u32,
}

fn default_consumer() -> Name {
    Name::from(format!("gpiod-p{}", std::process::id()).as_str())
}

impl Builder {
    /// Start building a new request.
    pub fn new() -> Self {
        Builder::default()
    }

    /// Start building a new request using the provided config.
    pub fn from_config(cfg: Config) -> Self {
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
    pub fn request(&self) -> Result<Request> {
        if self.chip.as_os_str().is_empty() {
            return Err(Error::InvalidArgument("No chip specified.".to_string()));
        }
        let chip = Chip::from_path(&self.chip)?;
        self.do_request(&chip).map(|f| self.to_request(f))
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_request(&self, chip: &Chip) -> Result<File> {
        match self.to_uapi()? {
            UapiRequest::Handle(hr) => v1::get_line_handle(&chip.f, hr)
                .map_err(|e| Error::UapiError(UapiCall::GetLineHandle, e)),
            UapiRequest::Event(er) => v1::get_line_event(&chip.f, er)
                .map_err(|e| Error::UapiError(UapiCall::GetLineEvent, e)),
            UapiRequest::Line(lr) => {
                v2::get_line(&chip.f, lr).map_err(|e| Error::UapiError(UapiCall::GetLine, e))
            }
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_request(&self, chip: &Chip) -> Result<File> {
        match self.to_uapi()? {
            UapiRequest::Handle(hr) => v1::get_line_handle(&chip.f, hr)
                .map_err(|e| Error::UapiError(UapiCall::GetLineHandle, e)),
            UapiRequest::Event(er) => v1::get_line_event(&chip.f, er)
                .map_err(|e| Error::UapiError(UapiCall::GetLineEvent, e)),
        }
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_request(&self, chip: &Chip) -> Result<File> {
        match self.to_uapi()? {
            UapiRequest::Line(lr) => {
                v2::get_line(&chip.f, lr).map_err(|e| Error::UapiError(UapiCall::GetLine, e))
            }
        }
    }

    fn to_request(&self, f: File) -> Request {
        Request {
            f,
            cfg: self.cfg.clone(),
        }
    }

    /// Replace the configuration.
    pub fn with_config(&mut self, cfg: Config) -> &mut Self {
        self.cfg = cfg;
        self
    }

    /// The current request configuration.
    ///
    /// Can be applied to other requests with [`with_config`].
    ///
    /// [`with_config`]: #method.with_config
    pub fn config(&self) -> &Config {
        &self.cfg
    }

    // mutators for attributes that cannot be changed after the request is made.

    /// Specify the consumer label to be applied to the request, and so to all lines in the request.
    ///
    /// If not specified, a label *"gpiod-p**pid**"*, where **pid** is the process id
    /// of the application, is applied by [`request`].
    ///
    /// [`request`]: #method.request
    pub fn with_consumer<N: Into<Name>>(&mut self, consumer: N) -> &mut Self {
        self.consumer = consumer.into();
        self
    }

    /// Set the event buffer size for edge events buffered in the kernel.
    ///
    /// This method is only required in unusual circumstances.
    ///
    /// The size provides a hint to the kernel as to the size of the buffer to store edge events
    /// until they are read from the Request.
    /// The default size is 16 times the number of lines in the request.  The kernel
    /// The default size is typically sufficient unless the hardware may generate bursts of events
    /// faster than the application can deal with them.
    pub fn with_event_buffer_size(&mut self, event_buffer_size: u32) -> &mut Self {
        self.event_buffer_size = event_buffer_size;
        self
    }

    /// Select the ABI version to use when requesting the lines and for subsequent operations.
    ///
    /// This is typically only required where an application supports both uAPI versions
    /// but the kernel may only support uAPI v1.
    ///
    /// The version to use can be determined by [`detect_abi_version`].
    ///
    /// [`detect_abi_version`]: fn@crate::detect_abi_version
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    pub fn using_abi_version(&mut self, abiv: AbiVersion) -> &mut Self {
        self.cfg.abiv = abiv;
        self
    }

    /// Set the chip from which to request lines.
    ///
    /// This applies to all lines in the request. It is not possible to request lines
    /// from different chips in the same request.
    ///
    /// The chip is identified by a path which must resolve to a GPIO character device.
    pub fn on_chip<P: Into<PathBuf>>(&mut self, path: P) -> &mut Self {
        self.chip = path.into();
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
        match self.cfg.abiv {
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
        if self.event_buffer_size != 0 {
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
            default_consumer().into()
        } else {
            self.consumer.clone().into()
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
            default_consumer().into()
        } else {
            self.consumer.clone().into()
        };
        Ok(UapiRequest::Line(v2::LineRequest {
            offsets: v2::Offsets::from_slice(&self.cfg.offsets),
            consumer,
            event_buffer_size: self.event_buffer_size,
            num_lines: self.cfg.offsets.len() as u32,
            config: self.cfg.to_v2()?,
            ..Default::default()
        }))
    }
}

#[derive(Debug)]
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
/// e.g.
/// ```
///    use gpiod::line::{Bias::*, Value::*};
///    use gpiod::request::Config;
///
///    let cfg = Config::new()
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
/// Note that the configuration is applied to hardware via a [`Builder.request`] or
/// [`Request.reconfigure`] call.  Any changes to the `Config` object, either before or after that
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

    /// The ABI version used to create the request, and so determines how to decode events.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    abiv: AbiVersion,
}
impl Config {
    pub fn new() -> Config {
        Config::default()
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
            cfg.direction = Direction::Input;
            // set output specific options back to default
            cfg.drive = None;
            cfg.value = None;
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
            cfg.direction = Direction::Output;
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
            cfg.direction = Direction::Input;
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
    pub fn with_direction(&mut self, direction: Direction) -> &mut Self {
        for cfg in self.selected_iter() {
            cfg.direction = direction;
            if cfg.direction == Direction::Output {
                // set input specific options back to default
                cfg.edge_detection = None;
                cfg.debounce_period = None;
            } else {
                // set output specific options back to default
                cfg.drive = None;
                cfg.value = None;
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
            cfg.direction = Direction::Output;
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
            cfg.direction = Direction::Input;
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
            base: top.base.clone(),
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
            if lcfg.direction == Direction::Output {
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
// This is strictly internal as external usage could invalidate the satefy contract.
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
pub struct Request {
    // some reference to the chip??
    f: File,
    cfg: Config,
}

impl Request {
    /// Get the values for a subset of the requested lines.
    ///
    /// The keys indicate the lines to get.
    /// If no keys are set then all requested lines are returned.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    pub fn values(&mut self, values: &mut Values) -> Result<()> {
        let offsets = &self.cfg.offsets;
        let res = match self.cfg.abiv {
            AbiVersion::V1 => {
                let mut vals = values.to_v1(offsets);
                v1::get_line_values(&self.f, &mut vals).map(|_| values.from_v1(offsets, &vals))
            }
            AbiVersion::V2 => {
                let mut vals = values.to_v2(offsets);
                v2::get_line_values(&self.f, &mut vals).map(|_| values.from_v2(offsets, &vals))
            }
        };
        res.map_err(|e| Error::UapiError(UapiCall::GetLineValues, e))
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    pub fn values(&mut self, values: &mut Values) -> Result<()> {
        let mut vals = values.to_uapi(&self.cfg.offsets);
        uapi::get_line_values(&self.f, &mut vals)
            .map(|_| values.from_uapi(&self.cfg.offsets, &vals))
            .map_err(|e| Error::UapiError(UapiCall::GetLineValues, e))
    }

    /// Get the value for one line in the request.
    pub fn value(&mut self, offset: Offset) -> Result<Value> {
        let mut values = Values::default();
        values.set(offset, Value::Inactive);
        self.values(&mut values)?;
        values.get(offset).ok_or_else(|| {
            Error::UnexpectedResponse("Requested value not present in response.".to_string())
        })
    }

    /// Set the values for a subset of the requested lines.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    pub fn set_values(&mut self, values: &Values) -> Result<()> {
        let res = match self.cfg.abiv {
            AbiVersion::V1 => v1::set_line_values(&self.f, &values.to_v1(&self.cfg.offsets)),
            AbiVersion::V2 => {
                let lv = &values.to_v2(&self.cfg.offsets);
                if lv.mask.is_empty() {
                    return Err(Error::InvalidArgument(
                        "No requested lines in set values.".to_string(),
                    ));
                }
                v2::set_line_values(&self.f, lv)
            }
        };
        res.map_err(|e| Error::UapiError(UapiCall::SetLineValues, e))
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    pub fn set_values(&mut self, values: &Values) -> Result<()> {
        uapi::set_line_values(&self.f, &values.to_uapi(&self.cfg.offsets))
            .map_err(|e| Error::UapiError(UapiCall::SetLineValues, e))
    }

    /// Set the value for one line in the request.
    pub fn set_value(&mut self, offset: Offset, value: Value) -> Result<()> {
        let mut values = Values::default();
        values.set(offset, value);
        self.set_values(&values)
    }

    /// The current request configuration.
    ///
    /// This is the configuration applied to the hardware.
    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// Reconfigure the request with the an updated configuration.
    ///
    /// Note that lines cannot be added or removed from the request.
    /// Any additional lines in new_cfg will be ignored, and any missing
    /// lines will retain their existing configuration.
    pub fn reconfigure(&mut self, new_cfg: &Config) -> Result<()> {
        let cfg = self.cfg.overlay(new_cfg);
        self.do_reconfigure(&cfg)?;
        // only update request config if reconfigure succeeds.
        self.cfg = cfg;
        Ok(())
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_reconfigure(&mut self, cfg: &Config) -> Result<()> {
        match self.cfg.abiv {
            AbiVersion::V1 => {
                if self.cfg.unique()?.edge_detection.is_some() {
                    return Err(Error::AbiLimitation(
                        AbiVersion::V1,
                        "cannot reconfigure lines with edge detection".to_string(),
                    ));
                }
                v1::set_line_config(&self.f, cfg.to_v1()?)
                    .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e))
            }
            AbiVersion::V2 => v2::set_line_config(&self.f, cfg.to_v2()?)
                .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e)),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_reconfigure(&mut self, cfg: &Config) -> Result<()> {
        if self.cfg.unique()?.edge_detection.is_some() {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "cannot reconfigure lines with edge detection".to_string(),
            ));
        }
        v1::set_line_config(&self.f, cfg.to_v1()?)
            .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e))
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_reconfigure(&mut self, cfg: &Config) -> Result<()> {
        v2::set_line_config(&self.f, cfg.to_v2()?)
            .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e))
    }

    /// Returns true when the request has edge events available to read.
    pub fn has_edge_event(&mut self) -> Result<bool> {
        gpiod_uapi::has_event(&mut self.f).map_err(|e| Error::UapiError(UapiCall::HasEvent, e))
    }

    /// Wait for an edge event to be available.
    pub fn wait_edge_event(&mut self, timeout: Duration) -> Result<bool> {
        gpiod_uapi::wait_event(&mut self.f, timeout)
            .map_err(|e| Error::UapiError(UapiCall::WaitEvent, e))
    }

    /// Read an edge event from a buffer.
    ///
    /// Assumes the buffer has been previously populated by a call to read on the request.
    pub fn edge_event_from_buf(&self, buf: &[u8]) -> Result<EdgeEvent> {
        self.do_edge_event_from_buf(buf)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_edge_event_from_buf(&self, buf: &[u8]) -> Result<EdgeEvent> {
        match self.cfg.abiv {
            AbiVersion::V1 => {
                let mut ee = EdgeEvent::from(
                    uapi::LineEdgeEvent::from_buf(buf)
                        .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
                );
                // populate offset for v1
                ee.offset = self.cfg.offsets[0];
                Ok(ee)
            }
            AbiVersion::V2 => Ok(EdgeEvent::from(
                uapi::LineEdgeEvent::from_buf(buf)
                    .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
            )),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_edge_event_from_buf(&self, buf: &[u8]) -> Result<EdgeEvent> {
        let mut ee = EdgeEvent::from(
            v1::LineEdgeEvent::from_buf(buf)
                .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
        );
        // populate offset for v1
        ee.offset = self.cfg.offsets[0]; // there can be only one
        Ok(ee)
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_edge_event_from_buf(&self, buf: &[u8]) -> Result<EdgeEvent> {
        Ok(EdgeEvent::from(
            v2::LineEdgeEvent::from_buf(buf)
                .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
        ))
    }

    /// The number of bytes required to buffer a single event read from the request.
    ///
    /// This can be used to size a [u8] buffer to read events into - the buffer size
    /// should be a multiple of this size.
    pub fn edge_event_size(&self) -> usize {
        self.do_edge_event_size()
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_edge_event_size(&self) -> usize {
        match self.cfg.abiv {
            AbiVersion::V1 => size_of::<v1::LineEdgeEvent>(),
            AbiVersion::V2 => size_of::<v2::LineEdgeEvent>(),
        }
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_edge_event_size(&self) -> usize {
        size_of::<uapi::LineEdgeEvent>()
    }

    /// Create an edge event buffer.
    ///
    /// * `capacity` - The number of events that can be buffered.
    pub fn new_edge_event_buffer(&self, capacity: usize) -> EdgeEventBuffer {
        let event_size = self.edge_event_size();
        let mut buf = Vec::with_capacity(capacity * event_size);
        buf.resize(buf.capacity(), 0);
        EdgeEventBuffer {
            event_size,
            filled: 0,
            read: 0,
            buf,
        }
    }
}
impl Read for Request {
    // This will read in v1:LineEdgeEvent or v2::LineEdgeEvent sized chunks so buf must be at least
    // as large as one event.
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.f.read(buf)
    }
}

/// A helper for reading edge events from a [`Request`].
///
/// Reads edge events from the kernel in bulk, where possible, while providing them
/// serially to the caller.
pub struct EdgeEventBuffer {
    /// The size of an individual edge event stored in the buffer.
    event_size: usize,
    /// The number of bytes currently written into the buffer
    filled: usize,
    /// The number of bytes currently read from the buffer.
    read: usize,
    /// The buffer for uAPI edge events, sized by event size and capacity
    buf: Vec<u8>,
}

impl EdgeEventBuffer {
    /// Returns true when either the buffer, or the request, has edge events available to read.
    pub fn has_event(&mut self, req: &mut Request) -> Result<bool> {
        if self.read < self.filled {
            return Ok(true);
        }
        req.has_edge_event()
    }
    /// Returns the next event from the buffer.
    ///
    /// If the buffer is empty then events are read from the request, which may block if no
    /// events are available. To avoid blocking, check [`has_event`] first, or use
    /// [`wait_event`] with a suitable timeout.
    ///
    /// [`has_event`]: #method.has_event
    /// [`wait_event`]: #method.wait_event
    pub fn read_event(&mut self, req: &mut Request) -> Result<EdgeEvent> {
        if self.read < self.filled {
            let evt_end = self.read + self.event_size;
            let evt = &self.buf[self.read..evt_end];
            self.read = evt_end;
            return req.edge_event_from_buf(evt);
        }
        self.read = 0;
        self.filled = 0;
        let n = req.read(&mut self.buf)?;
        // Could turn these into run-time errors, but they should never happen
        // so make them asserts to keep it simple.
        assert!(n > 0);
        assert_eq!(n % self.event_size, 0);
        self.filled = n;
        self.read = self.event_size;
        req.edge_event_from_buf(&self.buf[0..self.event_size])
    }
    /// Wait for an edge event from the request.
    ///
    /// * `timeout` - The maximum time to wait for an event.
    pub fn wait_event(&mut self, req: &mut Request, timeout: Duration) -> Result<EdgeEvent> {
        req.wait_edge_event(timeout)?;
        self.read_event(req)
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
    #[test]
    fn test_builder_new() {
        let b = Builder::new();
        assert_eq!(b.chip.as_os_str(), "");
        assert_eq!(b.cfg.num_lines(), 0);
        assert_eq!(b.consumer.as_os_str(), "");
        assert_eq!(b.event_buffer_size, 0);
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        assert_eq!(b.cfg.abiv, AbiVersion::V2);
    }
    #[test]
    fn test_builder_from_config() {
        let mut cfg = Config::new();
        cfg.as_input().with_lines(&[1, 7, 4]);
        let b = Builder::from_config(cfg);
        assert_eq!(b.cfg.offsets, &[1, 7, 4]);
        assert_eq!(b.cfg.num_lines(), 3);
    }
    #[test]
    fn test_builder_request() {
        let res = Builder::new().with_line(2).request();
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "No chip specified.");
    }
    #[test]
    fn test_builder_with_config() {
        let mut b = Builder::new();
        let mut cfg = Config::new();
        cfg.as_input().with_lines(&[1, 7, 4]);
        b.with_config(cfg);
        assert_eq!(b.cfg.offsets, &[1, 7, 4]);
        assert_eq!(b.cfg.num_lines(), 3);
    }
    #[test]
    fn test_builder_config() {
        let mut b = Builder::new();
        b.as_input().with_lines(&[1, 7, 4]);
        let cfg = b.config();
        assert_eq!(cfg.num_lines(), 3);
        assert_eq!(cfg.offsets, &[1, 7, 4]);
    }
    #[test]
    fn test_builder_with_consumer() {
        let mut b = Builder::new();
        assert_eq!(b.consumer.as_os_str(), "");
        b.with_consumer("builder test");
        assert_eq!(b.consumer.as_os_str(), "builder test");
    }
    #[test]
    fn test_builder_with_event_buffer_size() {
        let mut b = Builder::new();
        assert_eq!(b.event_buffer_size, 0);
        b.with_event_buffer_size(42);
        assert_eq!(b.event_buffer_size, 42);
    }
    #[test]
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn test_builder_using_abi_version() {
        let mut b = Builder::new();
        assert_eq!(b.cfg.abiv, AbiVersion::V2);
        b.using_abi_version(AbiVersion::V1);
        assert_eq!(b.cfg.abiv, AbiVersion::V1);
        b.using_abi_version(AbiVersion::V2);
        assert_eq!(b.cfg.abiv, AbiVersion::V2);
    }
    #[test]
    fn test_builder_on_chip() {
        let mut b = Builder::new();
        assert_eq!(b.chip.as_os_str(), "");
        b.on_chip("test chip");
        assert_eq!(b.chip.as_os_str(), "test chip");
    }
    #[test]
    fn test_builder_as_input() {
        let mut b = Builder::new();
        b.as_output(Active);
        assert_eq!(b.cfg.base.direction, Output);
        b.as_input();
        assert_eq!(b.cfg.base.direction, Input);
    }
    #[test]
    fn test_builder_as_output() {
        let mut b = Builder::new();
        b.as_input();
        assert_eq!(b.cfg.base.direction, Input);
        b.as_output(Active);
        assert_eq!(b.cfg.base.direction, Output);
    }
    #[test]
    fn test_builder_as_active_low() {
        let mut b = Builder::new();
        assert!(!b.cfg.base.active_low);
        b.as_active_low();
        assert!(b.cfg.base.active_low);
    }
    #[test]
    fn test_builder_as_active_high() {
        let mut b = Builder::new();
        assert!(!b.cfg.base.active_low);
        b.as_active_low();
        assert!(b.cfg.base.active_low);
        b.as_active_high();
        assert!(!b.cfg.base.active_low);
    }
    #[test]
    fn test_builder_with_bias() {
        let mut b = Builder::new();
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
    fn test_builder_with_debounce_period() {
        let d_us = Duration::from_micros(1234);
        let d_ns = Duration::from_nanos(234);
        let mut b = Builder::new();
        b.with_drive(OpenSource);
        assert_eq!(b.cfg.base.direction, Output);
        b.with_debounce_period(Duration::from_micros(1234));
        assert_eq!(b.cfg.base.debounce_period, Some(d_us));
        assert_eq!(b.cfg.base.direction, Input);
        b.with_debounce_period(Duration::from_nanos(234));
        assert_eq!(b.cfg.base.debounce_period, Some(d_ns));
        b.with_debounce_period(Duration::ZERO);
        assert!(b.cfg.base.debounce_period.is_none());
    }
    #[test]
    fn test_builder_with_direction() {
        let mut b = Builder::new();
        b.with_direction(Output);
        assert_eq!(b.cfg.base.direction, Output);
        b.with_direction(Input);
        assert_eq!(b.cfg.base.direction, Input);
    }
    #[test]
    fn test_builder_with_drive() {
        let mut b = Builder::new();
        b.with_bias(PullUp)
            .with_debounce_period(Duration::from_millis(10))
            .with_edge_detection(RisingEdge);
        assert_eq!(b.cfg.base.direction, Input);
        assert_eq!(b.cfg.base.bias, Some(PullUp));
        assert_eq!(b.cfg.base.debounce_period, Some(Duration::from_millis(10)));
        assert_eq!(b.cfg.base.edge_detection, Some(RisingEdge));
        b.with_drive(PushPull);
        assert_eq!(b.cfg.base.direction, Output);
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
    fn test_builder_with_edge_detection() {
        let mut b = Builder::new();
        b.with_drive(OpenSource);
        assert_eq!(b.cfg.base.direction, Output);
        b.with_edge_detection(RisingEdge);
        assert_eq!(b.cfg.base.edge_detection, Some(RisingEdge));
        assert_eq!(b.cfg.base.drive, None);
        assert_eq!(b.cfg.base.direction, Input);
        b.with_edge_detection(FallingEdge);
        assert_eq!(b.cfg.base.edge_detection, Some(FallingEdge));
        b.with_edge_detection(BothEdges);
        assert_eq!(b.cfg.base.edge_detection, Some(BothEdges));
    }
    #[test]
    fn test_builder_with_event_clock() {
        let mut b = Builder::new();
        assert_eq!(b.cfg.base.event_clock, None);
        b.with_event_clock(Realtime);
        assert_eq!(b.cfg.base.event_clock, Some(Realtime));
        b.with_event_clock(Monotonic);
        assert_eq!(b.cfg.base.event_clock, Some(Monotonic));
    }
    #[test]
    fn test_builder_with_line() {
        let mut b = Builder::new();
        assert_eq!(b.cfg.num_lines(), 0);
        b.with_line(3);
        assert_eq!(b.cfg.num_lines(), 1);
        b.with_line(5);
        assert_eq!(b.cfg.num_lines(), 2);
        b.with_line(3);
        assert_eq!(b.cfg.num_lines(), 2);
    }
    #[test]
    fn test_builder_without_line() {
        let mut b = Builder::new();
        b.with_lines(&[3, 1]);
        assert_eq!(b.cfg.num_lines(), 2);
        b.without_line(3);
        assert_eq!(b.cfg.num_lines(), 1);
        assert_eq!(b.cfg.offsets, &[1]);
    }
    #[test]
    fn test_builder_with_lines() {
        let mut b = Builder::new();
        assert_eq!(b.cfg.num_lines(), 0);
        b.with_lines(&[3, 1]);
        assert_eq!(b.cfg.num_lines(), 2);
        assert_eq!(b.cfg.offsets, &[3, 1]);
        b.with_lines(&[1, 5]);
        assert_eq!(b.cfg.num_lines(), 3);
        assert_eq!(b.cfg.offsets, &[3, 1, 5]);
    }
    #[test]
    fn test_builder_without_lines() {
        let mut b = Builder::new();
        b.with_lines(&[3, 1, 0, 5, 7]);
        assert_eq!(b.cfg.num_lines(), 5);
        b.without_lines(&[3, 5]);
        assert_eq!(b.cfg.num_lines(), 3);
        assert_eq!(b.cfg.offsets, &[1, 0, 7]);
    }
    #[test]
    fn test_builder_with_value() {
        let mut b = Builder::new();
        assert_eq!(b.cfg.base.value, None);
        b.as_input().with_value(Active);
        assert_eq!(b.cfg.base.value, Some(Active));
        b.as_input().with_value(Inactive);
        assert_eq!(b.cfg.base.value, Some(Inactive));
    }
    #[test]
    fn test_builder_from_line_config() {
        let mut b = Builder::new();
        let d_us = Duration::from_micros(1234);
        let lc = line::Config {
            direction: Output,
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
        assert_eq!(base.direction, Output);
        assert!(base.active_low);
        assert_eq!(base.direction, Output);
        assert_eq!(base.drive, Some(OpenSource));
        assert_eq!(base.edge_detection, Some(BothEdges));
        assert_eq!(base.event_clock, Some(Realtime));
        assert_eq!(base.value, Some(Active));
        assert_eq!(base.debounce_period, Some(d_us));
    }
    #[test]
    fn test_builder_to_uapi() {
        let res = Builder::new().to_uapi();
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "No lines specified.");
    }
    #[test]
    #[cfg(feature = "uapi_v1")]
    fn test_builder_to_v1() {
        let mut b = Builder::new();
        b.with_consumer("test builder")
            .with_lines(&[1, 8, 4])
            .as_active_low()
            .as_input();
        if let UapiRequest::Handle(hr) = b.to_v1().unwrap() {
            assert_eq!(hr.num_lines, 3);
            assert_eq!(hr.offsets.get(0), 1);
            assert_eq!(hr.offsets.get(1), 8);
            assert_eq!(hr.offsets.get(2), 4);
            assert_eq!(hr.consumer.as_os_str().to_string_lossy(), "test builder");
            assert!(hr
                .flags
                .contains(v1::HandleRequestFlags::INPUT | v1::HandleRequestFlags::ACTIVE_LOW));
            assert_eq!(hr.values.get(0), 0);
            assert_eq!(hr.values.get(1), 0);
            assert_eq!(hr.values.get(2), 0);
        } else {
            panic!("not a handle request");
        }

        let mut b = Builder::new();
        b.with_consumer("test builder")
            .with_line(8)
            .with_edge_detection(RisingEdge);
        if let UapiRequest::Event(er) = b.to_v1().unwrap() {
            assert_eq!(er.offset, 8);
            assert_eq!(er.consumer.as_os_str().to_string_lossy(), "test builder");
            assert!(er.handleflags.contains(v1::HandleRequestFlags::INPUT));
            assert!(er.eventflags.contains(v1::EventRequestFlags::RISING_EDGE));
        } else {
            panic!("not an event request");
        }

        let mut b = Builder::new();
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
                hr.consumer.as_os_str().to_string_lossy(),
                format!("gpiod-p{}", std::process::id()).as_str()
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

        let mut b = Builder::new();
        assert_eq!(
            b.with_event_buffer_size(42)
                .with_lines(&[1, 8, 4])
                .as_input()
                .to_v1()
                .unwrap_err()
                .to_string(),
            "uAPI ABI v1 does not support setting event buffer size."
        );

        let mut b = Builder::new();
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

        let mut b = Builder::new();
        assert_eq!(
            b.with_lines(&[1, 8, 4])
                .with_edge_detection(RisingEdge)
                .to_v1()
                .unwrap_err()
                .to_string(),
            "uAPI ABI v1 only supports edge detection on single line requests."
        );

        let mut b = Builder::new();
        assert_eq!(
            b.with_lines(&[1, 8, 4])
                .with_debounce_period(Duration::from_millis(23))
                .to_v1()
                .unwrap_err()
                .to_string(),
            "uAPI ABI v1 does not support debounce."
        );

        let mut b = Builder::new();
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
    fn test_builder_to_v2() {
        let mut b = Builder::new();
        b.with_consumer("test builder")
            .with_event_buffer_size(42)
            .with_lines(&[1, 8, 4])
            .with_edge_detection(RisingEdge)
            .with_event_clock(EventClock::Realtime);
        if let UapiRequest::Line(lr) = b.to_v2().unwrap() {
            assert_eq!(lr.num_lines, 3);
            assert_eq!(lr.offsets.get(0), 1);
            assert_eq!(lr.offsets.get(1), 8);
            assert_eq!(lr.offsets.get(2), 4);
            assert_eq!(lr.event_buffer_size, 42);
            assert_eq!(lr.consumer.as_os_str().to_string_lossy(), "test builder");
            assert!(lr.config.flags.contains(
                v2::LineFlags::INPUT
                    | v2::LineFlags::EDGE_RISING
                    | v2::LineFlags::EVENT_CLOCK_REALTIME
            ));
            assert_eq!(lr.config.num_attrs, 0);
        } else {
            panic!("not a line request");
        }

        let mut b = Builder::new();
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
                lr.consumer.as_os_str().to_string_lossy(),
                format!("gpiod-p{}", std::process::id()).as_str()
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
    #[test]
    fn test_config_default() {
        let cfg = Config::new();
        assert_eq!(cfg.lcfg.len(), 0);
        assert_eq!(cfg.selected.len(), 0);
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        assert_eq!(cfg.abiv, AbiVersion::V2);
        assert_eq!(cfg.base.direction, Input);
        assert!(!cfg.base.active_low);
        assert_eq!(cfg.base.direction, Input);
        assert_eq!(cfg.base.drive, None);
        assert_eq!(cfg.base.edge_detection, None);
        assert_eq!(cfg.base.event_clock, None);
        assert_eq!(cfg.base.event_clock, None);
        assert_eq!(cfg.base.value, None);
    }
    #[test]
    fn test_config_as_input() {
        let mut cfg = Config::new();
        cfg.as_output(Active).with_drive(OpenDrain);
        assert_eq!(cfg.base.direction, Output);
        assert_eq!(cfg.base.value, Some(Active));
        assert_eq!(cfg.base.drive, Some(OpenDrain));
        cfg.as_input();
        assert_eq!(cfg.base.direction, Input);
        assert_eq!(cfg.base.value, None);
        assert_eq!(cfg.base.drive, None);
    }
    #[test]
    fn test_config_as_output() {
        let mut cfg = Config::new();
        cfg.as_input().with_edge_detection(RisingEdge);
        assert_eq!(cfg.base.direction, Input);
        assert_eq!(cfg.base.value, None);
        assert_eq!(cfg.base.edge_detection, Some(RisingEdge));
        cfg.as_output(Active);
        assert_eq!(cfg.base.direction, Output);
        assert_eq!(cfg.base.value, Some(Active));
        assert_eq!(cfg.base.edge_detection, None);
    }
    #[test]
    fn test_config_as_active_low() {
        let mut cfg = Config::new();
        cfg.as_active_low();
        assert!(cfg.base.active_low);
        cfg.as_active_high();
        assert!(!cfg.base.active_low);
    }

    #[test]
    fn test_config_as_active_high() {
        let mut cfg = Config::new();
        cfg.as_active_low().with_line(3);
        assert!(cfg.lcfg.get(&3).unwrap().active_low);
        assert!(cfg.base.active_low);
        cfg.as_active_high();
        assert!(cfg.base.active_low);
        assert!(!cfg.lcfg.get(&3).unwrap().active_low);
    }
    #[test]
    fn test_config_with_bias() {
        let mut cfg = Config::new();
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
    fn test_config_with_debounce_period() {
        let d_us = Duration::from_micros(1234);
        let d_ns = Duration::from_nanos(234);
        let mut cfg = Config::new();
        cfg.with_drive(OpenSource);
        assert_eq!(cfg.base.direction, Output);
        cfg.with_debounce_period(Duration::from_micros(1234));
        assert_eq!(cfg.base.debounce_period, Some(d_us));
        assert_eq!(cfg.base.direction, Input);
        cfg.with_debounce_period(Duration::from_nanos(234));
        assert_eq!(cfg.base.debounce_period, Some(d_ns));
        cfg.with_debounce_period(Duration::ZERO);
        assert!(cfg.base.debounce_period.is_none());
    }
    #[test]
    fn test_config_with_direction() {
        let mut cfg = Config::new();
        cfg.with_direction(Output);
        assert_eq!(cfg.base.direction, Output);
        cfg.with_direction(Input);
        assert_eq!(cfg.base.direction, Input);
    }
    #[test]
    fn test_config_with_drive() {
        let mut cfg = Config::new();
        cfg.with_bias(PullUp)
            .with_debounce_period(Duration::from_millis(10))
            .with_edge_detection(RisingEdge);
        assert_eq!(cfg.base.direction, Input);
        assert_eq!(cfg.base.bias, Some(PullUp));
        assert_eq!(cfg.base.debounce_period, Some(Duration::from_millis(10)));
        assert_eq!(cfg.base.edge_detection, Some(RisingEdge));
        cfg.with_drive(PushPull);
        assert_eq!(cfg.base.direction, Output);
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
    fn test_config_with_edge_detection() {
        let mut cfg = Config::new();
        cfg.with_drive(OpenSource);
        assert_eq!(cfg.base.direction, Output);
        cfg.with_edge_detection(RisingEdge);
        assert_eq!(cfg.base.edge_detection, Some(RisingEdge));
        assert_eq!(cfg.base.drive, None);
        assert_eq!(cfg.base.direction, Input);
        cfg.with_edge_detection(FallingEdge);
        assert_eq!(cfg.base.edge_detection, Some(FallingEdge));
        cfg.with_edge_detection(BothEdges);
        assert_eq!(cfg.base.edge_detection, Some(BothEdges));
    }
    #[test]
    fn test_config_with_event_clock() {
        let mut cfg = Config::new();
        assert_eq!(cfg.base.event_clock, None);
        cfg.with_event_clock(Realtime);
        assert_eq!(cfg.base.event_clock, Some(Realtime));
        cfg.with_event_clock(Monotonic);
        assert_eq!(cfg.base.event_clock, Some(Monotonic));
    }
    #[test]
    fn test_config_with_line() {
        let mut cfg = Config::new();
        // initial mutator hits base config
        cfg.as_output(Active);
        assert_eq!(cfg.base.direction, Output);
        assert_eq!(cfg.base.value, Some(Active));
        cfg.as_input();
        assert_eq!(cfg.base.direction, Input);
        assert_eq!(cfg.base.value, None);
        assert!(cfg.offsets.is_empty());
        assert!(cfg.selected.is_empty());
        // mutator hits selected config
        cfg.with_line(3);
        assert_eq!(cfg.offsets, &[3]);
        assert_eq!(cfg.selected, &[3]);
        assert_eq!(cfg.lcfg.get(&3).unwrap().direction, Input);
        assert_eq!(cfg.lcfg.get(&3).unwrap().value, None);
        // mutator only hits selected config
        cfg.with_line(5);
        assert_eq!(cfg.offsets, &[3, 5]);
        assert_eq!(cfg.selected, &[5]);
        assert_eq!(cfg.lcfg.get(&5).unwrap().direction, Input);
        assert_eq!(cfg.lcfg.get(&5).unwrap().value, None);
        assert_eq!(cfg.lcfg.get(&3).unwrap().direction, Input);
        assert_eq!(cfg.lcfg.get(&3).unwrap().value, None);
        cfg.as_output(Inactive);
        assert_eq!(cfg.offsets, &[3, 5]);
        assert_eq!(cfg.selected, &[5]);
        assert_eq!(cfg.lcfg.get(&5).unwrap().direction, Output);
        assert_eq!(cfg.lcfg.get(&5).unwrap().value, Some(Inactive));
        assert_eq!(cfg.lcfg.get(&3).unwrap().direction, Input);
        assert_eq!(cfg.lcfg.get(&3).unwrap().value, None);
        assert_eq!(cfg.base.direction, Input);
        assert_eq!(cfg.base.value, None);
    }
    #[test]
    fn test_config_without_line() {
        let mut cfg = Config::new();
        cfg.as_output(Active);
        assert_eq!(cfg.base.direction, Output);
        assert_eq!(cfg.base.value, Some(Active));
        cfg.with_lines(&[1, 6, 2, 7]);
        assert_eq!(cfg.offsets, &[1, 6, 2, 7]);
        assert_eq!(cfg.selected, &[1, 6, 2, 7]);
        assert_eq!(cfg.lcfg.get(&2).unwrap().direction, Output);
        assert_eq!(cfg.lcfg.get(&2).unwrap().value, Some(Active));
        assert_eq!(cfg.lcfg.get(&6).unwrap().direction, Output);
        assert_eq!(cfg.lcfg.get(&6).unwrap().value, Some(Active));
        // from selected
        cfg.without_line(2);
        assert_eq!(cfg.offsets, &[1, 6, 7]);
        assert_eq!(cfg.selected, &[1, 6, 7]);
        assert_eq!(cfg.lcfg.get(&6).unwrap().direction, Output);
        assert_eq!(cfg.lcfg.get(&6).unwrap().value, Some(Active));
        cfg.with_line(1);
        assert_eq!(cfg.offsets, &[1, 6, 7]);
        assert_eq!(cfg.selected, &[1]);
        assert_eq!(cfg.lcfg.len(), 3);
        assert_eq!(cfg.lcfg.get(&6).unwrap().direction, Output);
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
    fn test_config_with_lines() {
        let mut cfg = Config::new();
        cfg.as_output(Active);
        assert_eq!(cfg.base.direction, Output);
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
    fn test_config_without_lines() {
        let mut cfg = Config::new();
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
    fn test_config_with_value() {
        let mut cfg = Config::new();
        assert_eq!(cfg.base.value, None);
        cfg.as_input().with_value(Active);
        assert_eq!(cfg.base.value, Some(Active));
        cfg.as_input().with_value(Inactive);
        assert_eq!(cfg.base.value, Some(Inactive));
    }
    #[test]
    fn test_config_from_line_config() {
        let d_us = Duration::from_micros(1234);
        let lc = line::Config {
            direction: Output,
            active_low: true,
            bias: Some(Disabled),
            drive: Some(OpenSource),
            edge_detection: Some(BothEdges),
            event_clock: Some(Realtime),
            debounce_period: Some(d_us),
            value: Some(Active),
        };
        // base
        let mut cfg = Config::new();
        cfg.from_line_config(&lc);
        assert!(cfg.lcfg.is_empty());
        assert!(cfg.selected.is_empty());
        assert_eq!(cfg.base.direction, Output);
        assert!(cfg.base.active_low);
        assert_eq!(cfg.base.drive, Some(OpenSource));
        assert_eq!(cfg.base.edge_detection, Some(BothEdges));
        assert_eq!(cfg.base.event_clock, Some(Realtime));
        assert_eq!(cfg.base.value, Some(Active));
        assert_eq!(cfg.base.debounce_period, Some(d_us));
        // select
        let mut cfg = Config::new();
        cfg.with_line(1).from_line_config(&lc);
        assert_eq!(cfg.selected, &[1]);
        assert_eq!(cfg.lcfg.len(), 1);
        let select = cfg.lcfg.get(&1).unwrap();
        assert_eq!(select.direction, Output);
        assert!(select.active_low);
        assert_eq!(select.direction, Output);
        assert_eq!(select.drive, Some(OpenSource));
        assert_eq!(select.edge_detection, Some(BothEdges));
        assert_eq!(select.event_clock, Some(Realtime));
        assert_eq!(select.value, Some(Active));
    }
    #[test]
    fn test_config_line_config() {
        let mut cfg = Config::new();
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
    fn test_config_lines() {
        let mut cfg = Config::new();
        cfg.with_lines(&[1, 2, 4, 6]).with_lines(&[2, 6, 9]);
        // should have 1,4 unselected and 2,6,9 select
        assert_eq!(cfg.lines(), &[1, 2, 4, 6, 9]);
        cfg.without_lines(&[1, 2]);
        assert_eq!(cfg.lines(), &[4, 6, 9]);
    }
    #[test]
    fn test_config_num_lines() {
        let mut cfg = Config::new();
        cfg.with_lines(&[1, 2, 4, 6]);
        assert_eq!(cfg.num_lines(), 4);
        cfg.with_lines(&[2, 6, 9]);
        // should have 1,4 unselected and 2,6,9 select
        assert_eq!(cfg.num_lines(), 5);
        cfg.without_lines(&[1, 2]);
    }
    #[test]
    #[cfg(feature = "uapi_v1")]
    fn test_config_unique() {
        let mut cfg = Config::new();
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
    fn test_config_overlay() {
        let mut bottom = Config::new();
        bottom.with_lines(&[1, 4, 7]).as_active_low();
        assert_eq!(bottom.num_lines(), 3);
        let mut top = Config::new();
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
    fn test_config_to_v1() {
        let mut cfg = Config::new();
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
    fn test_config_to_v1_values() {
        let mut cfg = Config::new();
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
    fn test_config_to_v2() {
        let mut cfg = Config::new();
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
