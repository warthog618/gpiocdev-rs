// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::line::{
    self, Bias, Direction, Drive, EdgeDetection, EventClock, Offset, Offsets, Value, Values,
};
use crate::{AbiVersion, Error, Result};
#[cfg(feature = "uapi_v2")]
use bitmaps::Bitmap;
#[cfg(feature = "uapi_v1")]
use gpiocdev_uapi::v1;
#[cfg(feature = "uapi_v2")]
use gpiocdev_uapi::v2;
use nohash_hasher::IntMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

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
    /// The path to the GPIO chip for all lines in the request.
    pub(super) chip: PathBuf,

    /// The base configuration that applies to a line when it is first added.
    ///
    /// Prior to adding lines this config is the receiver for all mutations.
    pub(super) base: line::Config,

    /// The configuration for the lines.
    pub(super) lcfg: IntMap<Offset, line::Config>,

    /// The set of lines described by this configuration, in order added.
    pub(super) offsets: Vec<Offset>,

    /// The current subset of lines being configured.
    ///
    /// If empty then the base config is selected.
    pub(super) selected: Vec<Offset>,
}

impl Config {
    /// Update the line configs using the updated config.
    ///
    /// Other fields are not updated as they are effectively immutable for the
    /// lifetime of the associated request.
    pub(super) fn update(&mut self, cfg: Config) {
        self.lcfg = cfg.lcfg;
    }

    /// Set the chip from which to request lines.
    ///
    /// This applies to all lines in the request. It is not possible to request lines
    /// from different chips in the same request.
    ///
    /// The chip is identified by a path which must resolve to a GPIO character device.
    ///
    ///
    pub fn on_chip<P: Into<PathBuf>>(&mut self, path: P) -> &mut Self {
        self.chip = path.into();
        self
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
    /// To determine the state of an existing output line, first request it [`as_is`],
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

    /// Add a found line to the config.
    ///
    /// The line must be on the same chip as any existing lines in the request.
    ///
    /// Note that all configuration mutators applied subsequently only apply to this line.
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::Result;
    /// # use gpiocdev::request::Config;
    /// # use gpiocdev::line::Value;
    /// # fn main() -> Result<()> {
    /// let led0 = gpiocdev::find_named_line("LED0").unwrap();
    /// let mut cfg = Config::default();
    /// cfg.with_found_line(&led0)?
    ///    .as_output(Value::Active);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_found_line(&mut self, line: &crate::FoundLine) -> Result<&mut Self> {
        if self.chip.as_os_str().is_empty() {
            self.on_chip(&line.chip);
        }
        if self.chip == line.chip {
            self.with_line(line.info.offset);
            Ok(self)
        } else {
            Err(Error::InvalidArgument(
                "Multiple chips requested.".to_string(),
            ))
        }
    }

    /// Add a set of found lines to the config.
    ///
    /// The lines must be on the same chip as any existing lines in the request.
    ///
    /// Note that all configuration mutators applied subsequently only apply to these lines.
    ///
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::Result;
    /// # use gpiocdev::request::Config;
    /// # use gpiocdev::line::EdgeDetection;
    /// # fn main() -> Result<()> {
    /// let buttons = gpiocdev::find_named_lines(&["BUTTON0","BUTTON1"], true)?;
    /// let mut cfg = Config::default();
    /// cfg.with_found_lines(&buttons)?
    ///    .with_edge_detection(EdgeDetection::BothEdges);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_found_lines(
        &mut self,
        lines: &HashMap<&str, crate::FoundLine>,
    ) -> Result<&mut Self> {
        for line in lines.values() {
            self.with_found_line(line)?;
        }
        for line in lines.values() {
            self.select_line(&line.info.offset);
        }
        Ok(self)
    }

    /// Add a line to the config.
    ///
    /// Note that all configuration mutators applied subsequently only apply to this line.
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

    /// Add a set of output lines, with values, to the config.
    ///
    /// Note that all configuration mutators applied subsequently only
    /// apply to this subset of lines.
    ///
    /// Passing empty values re-selects the base config for subsequent mutations.
    pub fn with_output_lines(&mut self, values: &Values) -> &mut Self {
        self.selected.clear();
        for (offset, value) in values.iter() {
            self.select_line(offset);
            let cfg = self.lcfg.get_mut(offset).unwrap();
            cfg.direction = Some(Direction::Output);
            cfg.value = Some(*value);
            // set input specific options back to default
            cfg.edge_detection = None;
            cfg.debounce_period = None;
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
    pub(crate) fn unique(&self) -> Result<&line::Config> {
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

    pub(crate) fn select_line(&mut self, offset: &Offset) {
        if !self.lcfg.contains_key(offset) {
            self.lcfg.insert(*offset, self.base.clone());
        }
        if self.selected.iter().all(|x| *x != *offset) {
            self.selected.push(*offset);
        }
        if self.offsets.iter().all(|x| *x != *offset) {
            self.offsets.push(*offset);
        }
    }

    // Overlay one config over another.
    // Used by reconfigure to update the request config.
    // New lines cannot be added, nor can any be removed.
    // If new lines are present in top they are ignored.
    // Any lines missing from top retain their existing config.
    pub(crate) fn overlay(&self, top: &Config) -> Config {
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
    pub(crate) fn to_v1(&self) -> Result<v1::HandleConfig> {
        let lcfg = self.unique()?;
        Ok(v1::HandleConfig {
            flags: lcfg.into(),
            values: self.to_v1_values()?,
            ..Default::default()
        })
    }

    #[cfg(feature = "uapi_v1")]
    pub(crate) fn to_v1_values(&self) -> Result<v1::LineValues> {
        let mut values = v1::LineValues::default();
        for (idx, offset) in self.offsets.iter().enumerate() {
            // unwrap is safe here as offsets match lcfg keys
            values.set(idx, self.lcfg.get(offset).unwrap().value().into());
        }
        Ok(values)
    }

    // v2
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    pub(crate) fn to_v2(&self) -> Result<v2::LineConfig> {
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
            let attr = cfg.attr_mut(num_attrs);
            attr.mask = *mask;
            attr.attr.set_flags(*flg);
            num_attrs += 1;
        }

        // outputs values, if any
        if !values.bits.is_empty() {
            let attr = cfg.attr_mut(num_attrs);
            attr.mask = values.mask;
            attr.attr.set_values(values.bits);
            num_attrs += 1;
        }

        // debounced
        for (dp, mask) in debounced.iter() {
            let attr = cfg.attr_mut(num_attrs);
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
    fn with_found_line() {
        let mut cfg = Config::default();

        // add one
        assert!(cfg.with_found_line(&3.into()).is_ok());
        assert_eq!(cfg.offsets, &[3]);
        assert_eq!(cfg.selected, &[3]);

        // and another
        assert!(cfg.with_found_line(&7.into()).is_ok());
        assert_eq!(cfg.offsets, &[3, 7]);
        assert_eq!(cfg.selected, &[7]);

        // and another
        assert!(cfg.with_found_line(&5.into()).is_ok());
        assert_eq!(cfg.offsets, &[3, 7, 5]);
        assert_eq!(cfg.selected, &[5]);

        // and a duplicate
        assert!(cfg.with_found_line(&7.into()).is_ok());
        assert_eq!(cfg.offsets, &[3, 7, 5]);
        assert_eq!(cfg.selected, &[7]);
    }

    fn sorted(s: &[u32]) -> Vec<u32> {
        let mut x = s.to_vec();
        x.sort();
        x
    }

    #[test]
    fn with_found_lines() {
        use crate::FoundLine;

        let mut cfg = Config::default();

        // add some
        let lines: HashMap<&str, FoundLine> = [("three", 3.into()), ("five", 5.into())]
            .iter()
            .cloned()
            .collect();
        assert!(cfg.with_found_lines(&lines).is_ok());
        assert_eq!(sorted(&cfg.offsets), &[3, 5]);
        assert_eq!(sorted(&cfg.selected), &[3, 5]);
        assert_eq!(cfg.lcfg.len(), 2);
        assert!(cfg.lcfg.contains_key(&3));
        assert!(cfg.lcfg.contains_key(&5));

        // add more
        let lines: HashMap<&str, FoundLine> = [("seven", 7.into()), ("one", 1.into())]
            .iter()
            .cloned()
            .collect();
        assert!(cfg.with_found_lines(&lines).is_ok());
        assert_eq!(sorted(&cfg.offsets), &[1, 3, 5, 7]);
        assert_eq!(sorted(&cfg.selected), &[1, 7]);
        assert_eq!(cfg.lcfg.len(), 4);
        assert!(cfg.lcfg.contains_key(&1));
        assert!(cfg.lcfg.contains_key(&3));
        assert!(cfg.lcfg.contains_key(&5));
        assert!(cfg.lcfg.contains_key(&7));

        // add duplicates
        let lines: HashMap<&str, FoundLine> = [("one", 1.into()), ("five", 5.into())]
            .iter()
            .cloned()
            .collect();
        assert!(cfg.with_found_lines(&lines).is_ok());
        assert_eq!(sorted(&cfg.offsets), &[1, 3, 5, 7]);
        assert_eq!(sorted(&cfg.selected), &[1, 5]);
        assert_eq!(cfg.lcfg.len(), 4);
        assert!(cfg.lcfg.contains_key(&1));
        assert!(cfg.lcfg.contains_key(&3));
        assert!(cfg.lcfg.contains_key(&5));
        assert!(cfg.lcfg.contains_key(&7));
    }

    #[test]
    fn with_output_lines() {
        let mut cfg = Config::default();
        let vv: Values = [(3, Value::Active), (5, Value::Inactive)]
            .into_iter()
            .collect();
        cfg.with_output_lines(&vv);
        assert_eq!(sorted(&cfg.offsets), &[3, 5]);
        assert_eq!(sorted(&cfg.selected), &[3, 5]);
        assert_eq!(cfg.lcfg.len(), 2);
        assert!(cfg.lcfg.contains_key(&3));
        assert_eq!(cfg.lcfg.get(&3).unwrap().direction, Some(Output));
        assert_eq!(cfg.lcfg.get(&3).unwrap().value, Some(Value::Active));

        assert!(cfg.lcfg.contains_key(&5));
        assert_eq!(cfg.lcfg.get(&5).unwrap().direction, Some(Output));
        assert_eq!(cfg.lcfg.get(&5).unwrap().value, Some(Value::Inactive));
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
