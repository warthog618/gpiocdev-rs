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
use gpiod_uapi::v1 as uv;
#[cfg(feature = "uapi_v1")]
use gpiod_uapi::v1;
#[cfg(feature = "uapi_v2")]
use gpiod_uapi::{v2, v2 as uv};
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
#[derive(Debug, Default, Clone)]
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
        let mut offsets = self.cfg.lines();
        offsets.sort_unstable(); // for predictability
        let mut cfg = self.cfg.clone();
        cfg.bank(); // for predictability
        Request { f, cfg, offsets }
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
    pub fn with_edge_detection(&mut self, edge: EdgeDetection) -> &mut Self {
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
                "does not support setting event buffer size.".to_string(),
            ));
        }
        match self.cfg.unique() {
            Err(_) => Err(Error::AbiLimitation(
                AbiVersion::V1,
                "requires all lines to share the same configuration.".to_string(),
            )),
            Ok(lcfg) => {
                if lcfg.debounce_period.is_some() {
                    return Err(Error::AbiLimitation(
                        AbiVersion::V1,
                        "does not support debounce.".to_string(),
                    ));
                }
                if lcfg.event_clock.is_some() {
                    return Err(Error::AbiLimitation(
                        AbiVersion::V1,
                        "does not support selecting the event clock source.".to_string(),
                    ));
                }
                let mut offsets = self.cfg.lines();
                offsets.sort_unstable(); // !!! perhaps allow client to specify ordering, or determine it from ordering of with_lines calls??
                let consumer = if self.consumer.is_empty() {
                    default_consumer().into()
                } else {
                    self.consumer.clone().into()
                };
                if lcfg.edge_detection.is_some() {
                    if offsets.len() != 1 {
                        return Err(Error::AbiLimitation(
                            AbiVersion::V1,
                            "only supports edge detection on single line requests.".to_string(),
                        ));
                    }
                    Ok(UapiRequest::Event(v1::EventRequest {
                        offset: offsets[0],
                        handleflags: lcfg.into(),
                        eventflags: lcfg.into(),
                        consumer,
                        ..Default::default()
                    }))
                } else {
                    Ok(UapiRequest::Handle(v1::HandleRequest {
                        offsets: v1::Offsets::from_slice(&offsets),
                        flags: lcfg.into(),
                        values: self.cfg.to_v1_values(&offsets)?,
                        consumer,
                        lines: offsets.len() as u32,
                        ..Default::default()
                    }))
                }
            }
        }
    }

    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    fn to_v2(&self) -> Result<UapiRequest> {
        let mut offsets = self.cfg.lines();
        offsets.sort_unstable(); // !!! perhaps allow client to specify ordering, or determine it from ordering of with_lines calls??
        let consumer = if self.consumer.is_empty() {
            default_consumer().into()
        } else {
            self.consumer.clone().into()
        };
        Ok(UapiRequest::Line(v2::LineRequest {
            offsets: v2::Offsets::from_slice(&offsets),
            consumer,
            event_buffer_size: self.event_buffer_size,
            num_lines: offsets.len() as u32,
            config: self.cfg.to_v2(&offsets)?,
            ..Default::default()
        }))
    }
}

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
#[derive(Debug, Clone)]
pub struct Config {
    /// The base configuration that applies to a line when it is first mentioned.
    ///
    /// Implemented as a map to be consistent with other fields, and to get an iterator
    /// that is compatible with them.
    ///
    /// Offset 0 is the only entry.
    ///
    /// Prior to adding lines this config is the receiver for all mutations.
    base: IntMap<Offset, line::Config>,

    // The configuration for the unselected lines.
    banked: IntMap<Offset, line::Config>,

    // The current set of lines being configured.  If empty then it is the base config.
    select: IntMap<Offset, line::Config>,

    /// The ABI version used to create the request, and so determines how to decode events.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    abiv: AbiVersion,
}
impl Default for Config {
    fn default() -> Config {
        let mut c = Config {
            base: Default::default(),
            banked: Default::default(),
            select: Default::default(),
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            abiv: Default::default(),
        };
        c.base.insert(0, Default::default());
        c
    }
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
        for cfg in self.select_iter() {
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
        for cfg in self.select_iter() {
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
        for cfg in self.select_iter() {
            cfg.active_low = true;
        }
        self
    }

    /// Set the selected lines to active high.
    ///
    /// This is the default active level setting.
    pub fn as_active_high(&mut self) -> &mut Self {
        for cfg in self.select_iter() {
            cfg.active_low = false;
        }
        self
    }

    /// Set the bias setting for the selected lines.
    pub fn with_bias<B: Into<Option<Bias>>>(&mut self, bias: B) -> &mut Self {
        let bias = bias.into();
        for cfg in self.select_iter() {
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
        for cfg in self.select_iter() {
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
        for cfg in self.select_iter() {
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
        for cfg in self.select_iter() {
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
        for cfg in self.select_iter() {
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
        for cfg in self.select_iter() {
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
        self.bank();
        self.select_line(offset);
        self
    }

    /// Remove a lines from the config.
    pub fn without_line(&mut self, offset: Offset) -> &mut Self {
        self.banked.remove(&offset);
        self.select.remove(&offset);
        self
    }

    /// Add a set of lines to the config.
    ///
    /// Note that all configuration mutators applied subsequently only
    /// apply to this subset of lines.
    ///
    /// Passing empty offsets re-selects the base config for subsequent mutations.
    pub fn with_lines(&mut self, offsets: &[Offset]) -> &mut Self {
        self.bank();
        for offset in offsets {
            self.select_line(*offset);
        }
        self
    }

    /// Remove a set of lines from the config.
    pub fn without_lines(&mut self, offsets: &[Offset]) -> &mut Self {
        for offset in offsets {
            self.banked.remove(offset);
            self.select.remove(offset);
        }
        self
    }

    /// Set the value of the selected lines.
    pub fn with_value(&mut self, value: Value) -> &mut Self {
        for cfg in self.select_iter() {
            cfg.value = Some(value);
        }
        self
    }

    /// Apply the configuration based on the snapshot from a single line.
    pub fn from_line_config(&mut self, lc: &line::Config) -> &mut Self {
        for cfg in self.select_iter() {
            *cfg = lc.clone();
        }
        self
    }

    /// Get the requested configuration for a particular line.
    ///
    /// This is the configuration that would be applied to the line if
    /// request were to be called.
    pub fn line_config(&self, offset: Offset) -> Option<&line::Config> {
        self.select
            .get(&offset)
            .or_else(|| self.banked.get(&offset))
    }

    /// Returns a snapshot of the offsets of the set of lines described by the Config.
    pub fn lines(&self) -> Offsets {
        let lines: Offsets = self
            .select
            .keys()
            .chain(self.banked.keys())
            .copied()
            .collect();
        lines
    }

    /// Returns the number of lines currently described by the Config.
    pub fn num_lines(&self) -> usize {
        self.select.len() + self.banked.len()
    }

    /// Returns the config that applies to all lines, or an error if the lines have
    /// distinct configurations.
    #[cfg(feature = "uapi_v1")]
    fn unique(&self) -> Result<&line::Config> {
        // have previously checked there is at least one line.
        let offsets = self.lines();
        // unwrap is safe as offset is drawn from either select or banked.
        let lcfg = self.line_config(offsets[0]).unwrap();
        if offsets.len() > 1 {
            for offset in offsets.iter().skip(1) {
                if !lcfg.equivalent(self.line_config(*offset).unwrap()) {
                    return Err(Error::AbiLimitation(
                        AbiVersion::V1,
                        "requires all lines to share the same configuration.".to_string(),
                    ));
                }
            }
        }
        Ok(lcfg)
    }

    // helper functions to simplify accessing and switching line selections.
    fn select_iter(&mut self) -> SelectIter {
        SelectIter::new(self)
    }

    fn bank(&mut self) {
        for (o, c) in self.select.iter() {
            self.banked.insert(*o, c.clone());
        }
        self.select.clear();
    }

    fn select_line(&mut self, offset: Offset) {
        let cfg = self
            .banked
            .remove(&offset)
            .unwrap_or_else(|| self.base.get(&0).cloned().unwrap());
        self.select.insert(offset, cfg);
    }

    // overlay one config over another.
    // Used by reconfigure to update the request config.
    // New lines cannot be added, nor can any be removed.
    // If new lines are present in top they are ignored.
    // Any lines missing from top retain their existing config.
    // The existing config should be banked as it is from an active request.
    fn overlay(&mut self, offsets: &[Offset], top: &Config) -> Config {
        assert!(self.select.is_empty(), "request config should be banked");
        let mut cfg = Config::new();
        cfg.base = top.base.clone();
        for offset in offsets {
            if let Some(lc) = top.line_config(*offset) {
                cfg.banked.insert(*offset, lc.clone());
            }
        }
        cfg
    }

    // conversions to uapi

    // v1
    #[cfg(feature = "uapi_v1")]
    fn to_v1(&self, offsets: &[Offset]) -> Result<v1::HandleConfig> {
        let lcfg = self.unique()?;
        if lcfg.edge_detection.is_some() {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "does not support reconfiguring lines with edge detection.".to_string(),
            ));
        }
        Ok(v1::HandleConfig {
            flags: lcfg.into(),
            values: self.to_v1_values(offsets)?,
            ..Default::default()
        })
    }

    #[cfg(feature = "uapi_v1")]
    fn to_v1_values(&self, offsets: &[Offset]) -> Result<v1::LineValues> {
        let mut values = v1::LineValues::default();
        for (idx, offset) in offsets.iter().enumerate() {
            values.set(idx, self.line_config(*offset).unwrap().value().into());
        }
        Ok(values)
    }

    // v2
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    fn to_v2(&self, offsets: &[Offset]) -> Result<v2::LineConfig> {
        // debounced and flags provide maps from attr values to bitmap of lines using those values.
        let mut debounced: IntMap<u32, LineSet> = IntMap::default();
        let mut flags: HashMap<v2::LineFlags, LineSet> = HashMap::default();
        let mut values: v2::LineValues = Default::default();
        for (idx, offset) in offsets.iter().enumerate() {
            // unwrap is safe here as offsets are derived from keys
            let lcfg = self.line_config(*offset).unwrap();
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

// A helper to iterate over the selected lines in a Config, or the base config
// if no lines are selected.
struct SelectIter<'a> {
    iter: std::collections::hash_map::IterMut<'a, Offset, line::Config>,
}

impl<'a> SelectIter<'a> {
    fn new(cfg: &'a mut Config) -> SelectIter {
        let iter = match cfg.select.is_empty() {
            true => cfg.base.iter_mut(),
            false => cfg.select.iter_mut(),
        };
        SelectIter { iter }
    }
}
impl<'a> Iterator for SelectIter<'a> {
    type Item = &'a mut line::Config;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|t| t.1)
    }
}

/// An active request of a set of lines.
pub struct Request {
    // some reference to the chip??
    f: File,
    offsets: Offsets, // drawn from Config on creation and then fixed.
    cfg: Config,
}

impl Request {
    /// Get the values for a subset of the requested lines.
    ///
    /// The keys indicate the lines to get.
    /// If no keys are set then all requested lines are returned.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    pub fn values(&mut self, values: &mut Values) -> Result<()> {
        let res = match self.cfg.abiv {
            AbiVersion::V1 => {
                let mut vals = values.to_v1(&self.offsets);
                v1::get_line_values(&self.f, &mut vals)
                    .map(|_| values.from_v1(&self.offsets, &vals))
            }
            AbiVersion::V2 => {
                let mut vals = values.to_v2(&self.offsets);
                v2::get_line_values(&self.f, &mut vals)
                    .map(|_| values.from_v2(&self.offsets, &vals))
            }
        };
        res.map_err(|e| Error::UapiError(UapiCall::GetLineValues, e))
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    pub fn values(&mut self, values: &mut Values) -> Result<()> {
        let mut vals = values.to_uapi(&self.offsets);
        uv::get_line_values(&self.f, &mut vals)
            .map(|_| values.from_uapi(&self.offsets, &vals))
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
            AbiVersion::V1 => v1::set_line_values(&self.f, &values.to_v1(&self.offsets)),
            AbiVersion::V2 => {
                let lv = &values.to_v2(&self.offsets);
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
        uv::set_line_values(&self.f, &values.to_uapi(&self.offsets))
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
    /// Any additional lines will be pruned from the provided config,
    /// and any missing lines will retain their existing configuration.
    pub fn reconfigure(&mut self, new_cfg: &Config) -> Result<()> {
        let cfg = self.cfg.overlay(&self.offsets, new_cfg);
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
                        "cannot reconfigure lines with edge detection.".to_string(),
                    ));
                }
                v1::set_line_config(&self.f, cfg.to_v1(&self.offsets)?)
                    .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e))
            }
            AbiVersion::V2 => v2::set_line_config(&self.f, cfg.to_v2(&self.offsets)?)
                .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e)),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_reconfigure(&mut self, cfg: &Config) -> Result<()> {
        v1::set_line_config(&self.f, cfg.to_v1(&self.offsets)?)
            .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e))
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_reconfigure(&mut self, cfg: &Config) -> Result<()> {
        v2::set_line_config(&self.f, cfg.to_v2(&self.offsets)?)
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
                    uv::LineEdgeEvent::from_buf(buf)
                        .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
                );
                // populate offset for v1
                ee.offset = self.offsets[0];
                Ok(ee)
            }
            AbiVersion::V2 => Ok(EdgeEvent::from(
                uv::LineEdgeEvent::from_buf(buf)
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
        ee.offset = self.offsets[0]; // there can be only one
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
        size_of::<uv::LineEdgeEvent>()
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
        // could turn these into run-time errors, but they should never happen
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
    fn test_config_default() {
        let cfg = Config::new();
        assert_eq!(cfg.base.len(), 1);
        assert_eq!(cfg.banked.len(), 0);
        assert_eq!(cfg.select.len(), 0);
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        assert_eq!(cfg.abiv, AbiVersion::V2);
        let base_cfg = cfg.base.get(&0).unwrap();
        assert_eq!(base_cfg.direction, Input);
        assert!(!base_cfg.active_low);
        assert_eq!(base_cfg.direction, Input);
        assert_eq!(base_cfg.drive, None);
        assert_eq!(base_cfg.edge_detection, None);
        assert_eq!(base_cfg.event_clock, None);
        assert_eq!(base_cfg.event_clock, None);
        assert_eq!(base_cfg.value, None);
    }
    #[test]
    fn test_config_as_input() {
        let mut cfg = Config::new();
        cfg.as_output(Active).with_drive(OpenDrain);
        let base_cfg = cfg.base.get(&0).unwrap();
        assert_eq!(base_cfg.direction, Output);
        assert_eq!(base_cfg.value, Some(Active));
        assert_eq!(base_cfg.drive, Some(OpenDrain));
        cfg.as_input();
        let base_cfg = cfg.base.get(&0).unwrap();
        assert_eq!(base_cfg.direction, Input);
        assert_eq!(base_cfg.value, None);
        assert_eq!(base_cfg.drive, None);
    }
    #[test]
    fn test_config_as_output() {
        let mut cfg = Config::new();
        cfg.as_input().with_edge_detection(RisingEdge);
        let base_cfg = cfg.base.get(&0).unwrap();
        assert_eq!(base_cfg.direction, Input);
        assert_eq!(base_cfg.value, None);
        assert_eq!(base_cfg.edge_detection, Some(RisingEdge));
        cfg.as_output(Active);
        let base_cfg = cfg.base.get(&0).unwrap();
        assert_eq!(base_cfg.direction, Output);
        assert_eq!(base_cfg.value, Some(Active));
        assert_eq!(base_cfg.edge_detection, None);
    }
    #[test]
    fn test_config_as_active_low() {
        let mut cfg = Config::new();
        cfg.as_active_low();
        assert!(cfg.base.get(&0).unwrap().active_low);
        cfg.as_active_high();
        assert!(!cfg.base.get(&0).unwrap().active_low);
    }

    #[test]
    fn test_config_as_active_high() {
        let mut cfg = Config::new();
        cfg.as_active_low().with_line(3);
        assert!(cfg.select.get(&3).unwrap().active_low);
        assert!(cfg.base.get(&0).unwrap().active_low);
        cfg.as_active_high();
        assert!(cfg.base.get(&0).unwrap().active_low);
        assert!(!cfg.select.get(&3).unwrap().active_low);
    }
    #[test]
    fn test_config_with_bias() {
        let mut cfg = Config::new();
        cfg.with_bias(PullUp);
        assert_eq!(cfg.base.get(&0).unwrap().bias, Some(PullUp));
        cfg.with_bias(PullDown);
        assert_eq!(cfg.base.get(&0).unwrap().bias, Some(PullDown));
        cfg.with_bias(Disabled);
        assert_eq!(cfg.base.get(&0).unwrap().bias, Some(Disabled));
        cfg.with_bias(None);
        assert_eq!(cfg.base.get(&0).unwrap().bias, None);
    }

    #[test]
    fn test_config_with_debounce() {
        let d_us = std::time::Duration::from_micros(1234);
        let d_ns = std::time::Duration::from_nanos(234);
        let mut cfg = Config::new();
        cfg.with_drive(OpenSource);
        assert_eq!(cfg.base.get(&0).unwrap().direction, Output);
        cfg.with_debounce_period(Duration::from_micros(1234));
        assert_eq!(cfg.base.get(&0).unwrap().debounce_period, Some(d_us));
        assert_eq!(cfg.base.get(&0).unwrap().direction, Input);
        cfg.with_debounce_period(Duration::from_nanos(234));
        assert_eq!(cfg.base.get(&0).unwrap().debounce_period, Some(d_ns));
        cfg.with_debounce_period(Duration::ZERO);
        assert!(cfg.base.get(&0).unwrap().debounce_period.is_none());
    }
    #[test]
    fn test_config_with_direction() {
        let mut cfg = Config::new();
        cfg.with_direction(Output);
        assert_eq!(cfg.base.get(&0).unwrap().direction, Output);
        cfg.with_direction(Input);
        assert_eq!(cfg.base.get(&0).unwrap().direction, Input);
    }
    #[test]
    fn test_config_with_drive() {
        let mut cfg = Config::new();
        cfg.with_bias(PullUp)
            .with_debounce_period(Duration::from_millis(10))
            .with_edge_detection(EdgeDetection::RisingEdge);
        assert_eq!(cfg.base.get(&0).unwrap().direction, Input);
        assert_eq!(cfg.base.get(&0).unwrap().bias, Some(PullUp));
        assert_eq!(
            cfg.base.get(&0).unwrap().debounce_period,
            Some(Duration::from_millis(10))
        );
        assert_eq!(cfg.base.get(&0).unwrap().edge_detection, Some(RisingEdge));
        cfg.with_drive(PushPull);
        assert_eq!(cfg.base.get(&0).unwrap().direction, Output);
        assert_eq!(cfg.base.get(&0).unwrap().drive, Some(PushPull));
        assert_eq!(cfg.base.get(&0).unwrap().bias, Some(PullUp));
        assert!(cfg.base.get(&0).unwrap().debounce_period.is_none());
        assert_eq!(cfg.base.get(&0).unwrap().edge_detection, None);
        cfg.with_drive(OpenDrain);
        assert_eq!(cfg.base.get(&0).unwrap().drive, Some(OpenDrain));
        cfg.with_drive(OpenSource);
        assert_eq!(cfg.base.get(&0).unwrap().drive, Some(OpenSource));
    }
    #[test]
    fn test_config_with_edge_detection() {
        let mut cfg = Config::new();
        cfg.with_drive(OpenSource);
        assert_eq!(cfg.base.get(&0).unwrap().direction, Output);
        cfg.with_edge_detection(RisingEdge);
        assert_eq!(cfg.base.get(&0).unwrap().edge_detection, Some(RisingEdge));
        assert_eq!(cfg.base.get(&0).unwrap().drive, None);
        assert_eq!(cfg.base.get(&0).unwrap().direction, Input);
        cfg.with_edge_detection(FallingEdge);
        assert_eq!(cfg.base.get(&0).unwrap().edge_detection, Some(FallingEdge));
        cfg.with_edge_detection(BothEdges);
        assert_eq!(cfg.base.get(&0).unwrap().edge_detection, Some(BothEdges));
    }
    #[test]
    fn test_config_with_event_clock() {
        let mut cfg = Config::new();
        assert_eq!(cfg.base.get(&0).unwrap().event_clock, None);
        cfg.with_event_clock(Realtime);
        assert_eq!(cfg.base.get(&0).unwrap().event_clock, Some(Realtime));
        cfg.with_event_clock(Monotonic);
        assert_eq!(cfg.base.get(&0).unwrap().event_clock, Some(Monotonic));
    }
    #[test]
    fn test_config_with_line() {
        let mut cfg = Config::new();
        // initial mutator hits base_config
        cfg.as_output(Active);
        assert_eq!(cfg.base.get(&0).unwrap().direction, Output);
        assert_eq!(cfg.base.get(&0).unwrap().value, Some(Active));
        cfg.as_input();
        assert_eq!(cfg.base.get(&0).unwrap().direction, Input);
        assert_eq!(cfg.base.get(&0).unwrap().value, None);
        // base_config fixed, mutator hits select_config
        cfg.with_line(3);
        assert_eq!(cfg.select.len(), 1);
        assert_eq!(cfg.select.get(&3).unwrap().direction, Input);
        assert_eq!(cfg.select.get(&3).unwrap().value, None);
        // base_config fixed, line 3 fixed in line_config, mutator hits select_config
        cfg.with_line(5);
        assert_eq!(cfg.select.get(&5).unwrap().direction, Input);
        assert_eq!(cfg.select.get(&5).unwrap().value, None);
        assert_eq!(cfg.banked.get(&3).unwrap().direction, Input);
        assert_eq!(cfg.banked.get(&3).unwrap().value, None);
        cfg.as_output(Inactive);
        assert_eq!(cfg.select.get(&5).unwrap().direction, Output);
        assert_eq!(cfg.select.get(&5).unwrap().value, Some(Inactive));
        assert_eq!(cfg.banked.get(&3).unwrap().direction, Input);
        assert_eq!(cfg.banked.get(&3).unwrap().value, None);
        assert_eq!(cfg.base.get(&0).unwrap().direction, Input);
        assert_eq!(cfg.base.get(&0).unwrap().value, None);
    }
    #[test]
    fn test_config_without_line() {
        let mut cfg = Config::new();
        cfg.as_output(Active);
        assert_eq!(cfg.base.get(&0).unwrap().direction, Output);
        assert_eq!(cfg.base.get(&0).unwrap().value, Some(Active));
        cfg.with_lines(&[1, 2, 6, 7]);
        assert_eq!(cfg.select.len(), 4);
        assert_eq!(cfg.select.get(&2).unwrap().direction, Output);
        assert_eq!(cfg.select.get(&2).unwrap().value, Some(Active));
        assert_eq!(cfg.select.get(&6).unwrap().direction, Output);
        assert_eq!(cfg.select.get(&6).unwrap().value, Some(Active));
        // from select
        cfg.without_line(2);
        assert_eq!(cfg.select.len(), 3);
        assert_eq!(cfg.select.get(&6).unwrap().direction, Output);
        assert_eq!(cfg.select.get(&6).unwrap().value, Some(Active));
        cfg.with_line(1);
        assert_eq!(cfg.select.len(), 1);
        assert_eq!(cfg.banked.len(), 2);
        assert_eq!(cfg.banked.get(&6).unwrap().direction, Output);
        assert_eq!(cfg.banked.get(&6).unwrap().value, Some(Active));
        // from banked
        cfg.without_line(6);
        assert_eq!(cfg.select.len(), 1);
        assert_eq!(cfg.banked.len(), 1);
        assert!(cfg.select.get(&1).is_some());
        assert!(cfg.banked.get(&7).is_some());
        assert!(cfg.banked.get(&6).is_none());
        // last select
        cfg.without_line(1);
        assert_eq!(cfg.select.len(), 0);
        assert_eq!(cfg.banked.len(), 1);
        assert!(cfg.select.get(&1).is_none());
    }
    #[test]
    fn test_config_with_lines() {
        let mut cfg = Config::new();
        cfg.as_output(Active);
        assert_eq!(cfg.base.get(&0).unwrap().direction, Output);
        assert_eq!(cfg.base.get(&0).unwrap().value, Some(Active));
        // select some
        cfg.with_lines(&[1, 2, 6, 7]);
        assert_eq!(cfg.banked.len(), 0);
        assert_eq!(cfg.select.len(), 4);
        assert!(cfg.select.get(&1).is_some());
        assert!(cfg.select.get(&2).is_some());
        assert!(cfg.select.get(&6).is_some());
        assert!(cfg.select.get(&7).is_some());
        // bank some, add another
        cfg.with_lines(&[1, 2, 9]);
        assert_eq!(cfg.banked.len(), 2);
        assert_eq!(cfg.select.len(), 3);
        assert!(cfg.banked.get(&6).is_some());
        assert!(cfg.banked.get(&7).is_some());
        assert!(cfg.select.get(&1).is_some());
        assert!(cfg.select.get(&2).is_some());
        assert!(cfg.select.get(&9).is_some());
        // bank all, select none
        cfg.with_lines(&[]);
        assert_eq!(cfg.banked.len(), 5);
        assert_eq!(cfg.select.len(), 0);
        assert!(cfg.banked.get(&1).is_some());
        assert!(cfg.banked.get(&2).is_some());
        assert!(cfg.banked.get(&6).is_some());
        assert!(cfg.banked.get(&7).is_some());
        assert!(cfg.banked.get(&9).is_some());
    }
    #[test]
    fn test_config_without_lines() {
        let mut cfg = Config::new();
        cfg.with_lines(&[1, 2, 3, 4]).with_lines(&[5, 6, 7, 8]);
        // from banked and select
        cfg.without_lines(&[1, 4, 6, 7]);
        assert_eq!(cfg.banked.len(), 2);
        assert_eq!(cfg.select.len(), 2);
        assert!(cfg.banked.get(&2).is_some());
        assert!(cfg.banked.get(&3).is_some());
        assert!(cfg.select.get(&5).is_some());
        assert!(cfg.select.get(&8).is_some());
    }
    #[test]
    fn test_config_with_value() {
        let mut cfg = Config::new();
        assert_eq!(cfg.base.get(&0).unwrap().value, None);
        cfg.as_input().with_value(Active);
        assert_eq!(cfg.base.get(&0).unwrap().value, Some(Active));
        cfg.as_input().with_value(Inactive);
        assert_eq!(cfg.base.get(&0).unwrap().value, Some(Inactive));
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
        assert_eq!(cfg.base.len(), 1);
        assert_eq!(cfg.banked.len(), 0);
        assert_eq!(cfg.select.len(), 0);
        let base = cfg.base.get(&0).unwrap();
        assert_eq!(base.direction, Output);
        assert!(base.active_low);
        assert_eq!(base.direction, Output);
        assert_eq!(base.drive, Some(OpenSource));
        assert_eq!(base.edge_detection, Some(BothEdges));
        assert_eq!(base.event_clock, Some(Realtime));
        assert_eq!(base.value, Some(Active));
        assert_eq!(base.debounce_period, Some(d_us));
        // select
        let mut cfg = Config::new();
        cfg.with_line(1).from_line_config(&lc);
        assert_eq!(cfg.base.len(), 1);
        assert_eq!(cfg.banked.len(), 0);
        assert_eq!(cfg.select.len(), 1);
        let select = cfg.select.get(&1).unwrap();
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
        // from banked
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
        // should have 1,4 banked and 2,6,9 select
        let mut lines = cfg.lines();
        assert_eq!(lines.len(), 5);
        // should be merged, but mnay be unsorted
        lines.sort_unstable();
        assert_eq!(lines, &[1, 2, 4, 6, 9]);
    }
    #[test]
    fn test_builder_request() {
        let b = Builder::new();
        let res = b.request();
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "No chip specified.");
    }
}
