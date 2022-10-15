// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

#[cfg(all(feature = "uapi_v1", not(feature = "uapi_v2")))]
use gpiocdev_uapi::v1 as uapi;
#[cfg(feature = "uapi_v1")]
use gpiocdev_uapi::v1;
#[cfg(feature = "uapi_v2")]
use gpiocdev_uapi::{v2, v2 as uapi};
use nohash_hasher::IntMap;
use std::collections::hash_map::Iter;
use std::time::Duration;

/// The configuration settings for a single line.
///
// Note it does not contain the offset to allow it to be applied to multiple lines.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Config {
    /// The direction setting for the line.
    pub direction: Option<Direction>,

    /// The active low setting for the line.
    pub active_low: bool,

    /// The bias setting for the line.
    pub bias: Option<Bias>,

    /// The drive setting for the line.
    ///
    /// Only relevant for output lines.
    pub drive: Option<Drive>,

    /// The edge detection setting for the line.
    ///
    /// Only relevant for input lines.
    pub edge_detection: Option<EdgeDetection>,

    /// The source clock for edge event timestamps.
    ///
    /// Only relevant for input lines with edge detection enabled.
    pub event_clock: Option<EventClock>,

    /// The debounce period.
    ///
    /// Setting the debounce period filters edges occuring at a rate faster than
    /// that period.
    ///
    /// Only relevant for input lines with edge detection enabled.
    pub debounce_period: Option<Duration>,

    /// The logical value to be applied to the line if it is an output.
    pub value: Option<Value>,
}

impl Config {
    /// Check that two configs are equivalent, ignoring the line value.
    #[cfg(feature = "uapi_v1")]
    pub(crate) fn equivalent(&self, right: &Config) -> bool {
        if (self.direction == right.direction)
            && (self.active_low == right.active_low)
            && (self.bias == right.bias)
            && (self.drive == right.drive)
            && (self.edge_detection == right.edge_detection)
            && (self.event_clock == right.event_clock)
            && (self.debounce_period == right.debounce_period)
        {
            return true;
        }
        false
    }

    /// return the effective value specified by the config
    pub(crate) fn value(&self) -> Value {
        match self.value {
            None => Value::Inactive,
            Some(x) => x,
        }
    }
}

#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
impl From<&Config> for v2::LineFlags {
    fn from(cfg: &Config) -> v2::LineFlags {
        let mut flags = v2::LineFlags::default();
        if cfg.active_low {
            flags.set(v2::LineFlags::ACTIVE_LOW, true);
        }
        match cfg.bias {
            None => {}
            Some(Bias::PullUp) => flags.set(v2::LineFlags::BIAS_PULL_UP, true),
            Some(Bias::PullDown) => flags.set(v2::LineFlags::BIAS_PULL_DOWN, true),
            Some(Bias::Disabled) => flags.set(v2::LineFlags::BIAS_DISABLED, true),
        };
        match cfg.direction {
            None => {}
            Some(Direction::Output) => {
                flags.set(v2::LineFlags::OUTPUT, true);
                match cfg.drive {
                    None => {}
                    Some(Drive::PushPull) => {}
                    Some(Drive::OpenDrain) => flags.set(v2::LineFlags::OPEN_DRAIN, true),
                    Some(Drive::OpenSource) => flags.set(v2::LineFlags::OPEN_SOURCE, true),
                };
            }
            Some(Direction::Input) => {
                flags.set(v2::LineFlags::INPUT, true);
                match cfg.edge_detection {
                    None => {}
                    Some(EdgeDetection::RisingEdge) => flags.set(v2::LineFlags::EDGE_RISING, true),
                    Some(EdgeDetection::FallingEdge) => {
                        flags.set(v2::LineFlags::EDGE_FALLING, true)
                    }
                    Some(EdgeDetection::BothEdges) => flags.set(
                        v2::LineFlags::EDGE_RISING | v2::LineFlags::EDGE_FALLING,
                        true,
                    ),
                };
                if cfg.edge_detection.is_some() {
                    match cfg.event_clock {
                        None => {}
                        Some(EventClock::Monotonic) => {}
                        Some(EventClock::Realtime) => {
                            flags.set(v2::LineFlags::EVENT_CLOCK_REALTIME, true)
                        }
                        Some(EventClock::Hte) => flags.set(v2::LineFlags::EVENT_CLOCK_HTE, true),
                    };
                }
            }
        };
        flags
    }
}

#[cfg(feature = "uapi_v1")]
impl From<&Config> for v1::EventRequestFlags {
    fn from(cfg: &Config) -> v1::EventRequestFlags {
        use EdgeDetection::*;
        match cfg.edge_detection {
            Some(RisingEdge) => v1::EventRequestFlags::RISING_EDGE,
            Some(FallingEdge) => v1::EventRequestFlags::FALLING_EDGE,
            Some(BothEdges) => v1::EventRequestFlags::BOTH_EDGES,
            None => v1::EventRequestFlags::default(),
        }
    }
}

#[cfg(feature = "uapi_v1")]
impl From<&Config> for v1::HandleRequestFlags {
    fn from(cfg: &Config) -> v1::HandleRequestFlags {
        let mut flags = v1::HandleRequestFlags::default();
        match cfg.direction {
            None => {}
            Some(Direction::Input) => flags.set(v1::HandleRequestFlags::INPUT, true),
            Some(Direction::Output) => {
                flags.set(v1::HandleRequestFlags::OUTPUT, true);
                match cfg.drive {
                    None => {}
                    Some(Drive::PushPull) => {}
                    Some(Drive::OpenDrain) => flags.set(v1::HandleRequestFlags::OPEN_DRAIN, true),
                    Some(Drive::OpenSource) => flags.set(v1::HandleRequestFlags::OPEN_SOURCE, true),
                };
            }
        };
        if cfg.active_low {
            flags.set(v1::HandleRequestFlags::ACTIVE_LOW, true);
        }
        match cfg.bias {
            None => {}
            Some(Bias::PullUp) => flags.set(v1::HandleRequestFlags::BIAS_PULL_UP, true),
            Some(Bias::PullDown) => flags.set(v1::HandleRequestFlags::BIAS_PULL_DOWN, true),
            Some(Bias::Disabled) => flags.set(v1::HandleRequestFlags::BIAS_DISABLED, true),
        };
        flags
    }
}

/// The publicly available information for a line.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Info {
    /// The line offset on the GPIO chip.
    pub offset: Offset,

    /// The name of this GPIO line, such as the output pin of the line on
    /// the chip, a rail or a pin header name on a board, as specified by the
    /// GPIO chip.
    ///
    /// May be empty.
    pub name: String,

    /// A functional name for the consumer of this GPIO line as set
    /// by whatever is using it.
    ///
    /// May be empty if not set by the user or the line is unused.
    pub consumer: String,

    /// When true the line is used and not available for request.
    pub used: bool,

    /// When true the line active state corresponds to a physical low.
    pub active_low: bool,

    /// The direction of the line.
    pub direction: Direction,

    /// The bias state of the line.
    pub bias: Option<Bias>,

    /// The drive applied to output lines.
    ///
    /// Only relevant for output lines.
    pub drive: Option<Drive>,

    /// The edge detection state for the line.
    ///
    /// Only relevant for input lines.
    pub edge_detection: Option<EdgeDetection>,

    /// The source clock for edge event timestamps.
    ///
    /// Only relevant for input lines with edge detection.
    pub event_clock: Option<EventClock>,

    /// The debounce period.
    ///
    /// Only relevant for input lines with edge detection.
    ///
    /// None or a zero value means no debounce.
    pub debounce_period: Option<Duration>,
}

#[cfg(feature = "uapi_v1")]
impl From<&v1::LineInfo> for Info {
    fn from(li: &v1::LineInfo) -> Self {
        Info {
            offset: li.offset,
            name: String::from(&li.name),
            consumer: String::from(&li.consumer),
            used: li.flags.contains(v1::LineInfoFlags::USED),
            active_low: li.flags.contains(v1::LineInfoFlags::ACTIVE_LOW),
            direction: Direction::from(li.flags),
            bias: Bias::try_from(li.flags).ok(),
            drive: Drive::try_from(li.flags).ok(),
            // other fields are unknown to v1
            ..Default::default()
        }
    }
}
#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
impl From<&v2::LineInfo> for Info {
    fn from(li: &v2::LineInfo) -> Self {
        let mut debounce_period = None;
        for idx in 0..li.num_attrs as usize {
            if let Some(v2::LineAttributeValue::DebouncePeriod(db)) = li.attr(idx).to_value() {
                debounce_period = Some(db);
            }
        }
        let ed = EdgeDetection::try_from(li.flags).ok();
        let ec = if ed.is_some() {
            EventClock::try_from(li.flags).ok()
        } else {
            None
        };
        Info {
            offset: li.offset,
            name: String::from(&li.name),
            consumer: String::from(&li.consumer),
            used: li.flags.contains(v2::LineFlags::USED),
            active_low: li.flags.contains(v2::LineFlags::ACTIVE_LOW),
            direction: Direction::from(li.flags),
            bias: Bias::try_from(li.flags).ok(),
            drive: Drive::try_from(li.flags).ok(),
            edge_detection: ed,
            event_clock: ec,
            debounce_period,
        }
    }
}

/// An identifier for a line on a particular chip.
///
/// Valid offsets are in the range 0..`num_lines` as reported in the chip [`Info`](super::chip::Info).
pub type Offset = u32;

/// A collection of line offsets.
pub type Offsets = Vec<Offset>;

/// The logical level of a line.
///
/// The mapping between logical and physical levels depends on the
///  active-low setting as follows:
///
/// |             | Physical Low | Physical High |
/// |-------------|--------------|---------------|
/// | **Active-High** | Inactive | Active |
/// | **Active-Low**  | Active | Inactive |
///
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Value {
    /// The line is inactive.
    Inactive,
    /// The line is active.
    Active,
}
impl Value {
    pub fn not(&self) -> Value {
        match self {
            Value::Active => Value::Inactive,
            Value::Inactive => Value::Active,
        }
    }
}
impl Default for Value {
    fn default() -> Self {
        Self::Inactive
    }
}
impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Value::Active => "active",
            Value::Inactive => "inactive",
        };
        write!(f, "{}", s)
    }
}

impl From<Value> for bool {
    fn from(l: Value) -> bool {
        match l {
            Value::Inactive => false,
            Value::Active => true,
        }
    }
}
impl From<Value> for u8 {
    fn from(l: Value) -> u8 {
        match l {
            Value::Inactive => 0,
            Value::Active => 1,
        }
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Value {
        match b {
            false => Value::Inactive,
            true => Value::Active,
        }
    }
}
impl From<u8> for Value {
    fn from(i: u8) -> Value {
        match i {
            0 => Value::Inactive,
            _ => Value::Active,
        }
    }
}

/// A  collection of line values.
///
/// Lines are identified by their offset.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Values(IntMap<Offset, Value>);
impl Values {
    /// overlays the values from src over the values in the dst.
    #[cfg(feature = "uapi_v1")]
    pub(crate) fn overlay_from_v1(&mut self, offsets: &[Offset], src: &v1::LineValues) {
        for (idx, offset) in offsets.iter().enumerate() {
            self.0.insert(*offset, src.get(idx).into());
        }
    }
    /// overlays the values from src over the values in the dst.
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    pub(crate) fn overlay_from_v2(&mut self, offsets: &[Offset], src: &v2::LineValues) {
        for (idx, offset) in offsets.iter().enumerate() {
            if let Some(val) = src.get(idx) {
                self.0.insert(*offset, val.into());
            }
        }
    }

    // v1 values are a contiguous list.  If a list shorter than offsets
    // is presented to the kernel then the missing lines default to zero.
    // Build the complete values list here with any missing values being zero filled.
    #[cfg(feature = "uapi_v1")]
    pub(crate) fn to_v1(&self, offsets: &[Offset]) -> v1::LineValues {
        let mut dst: v1::LineValues = Default::default();
        for (idx, offset) in offsets.iter().enumerate() {
            if let Some(val) = self.0.get(offset) {
                dst.set(idx, (*val).into());
            }
        }
        dst
    }
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    pub(crate) fn to_v2(&self, offsets: &[Offset]) -> v2::LineValues {
        let mut dst: v2::LineValues = Default::default();
        for (idx, offset) in offsets.iter().enumerate() {
            if self.is_empty() {
                dst.set(idx, false);
            } else if let Some(val) = self.0.get(offset) {
                dst.set(idx, (*val).into());
            }
        }
        dst
    }

    /// Get the value of a line.
    #[inline]
    pub fn get(&self, offset: Offset) -> Option<Value> {
        self.0.get(&offset).copied()
    }

    /// Set the value of a line.
    #[inline]
    pub fn set(&mut self, offset: Offset, val: Value) {
        self.0.insert(offset, val);
    }

    /// Toggle the value of a line.
    ///
    /// If not already set then sets the line active.
    #[inline]
    pub fn toggle(&mut self, offset: Offset) {
        let val = self.0.get(&offset).copied().unwrap_or(Value::Inactive);
        self.0.insert(offset, val.not());
    }

    /// Remove any value setting for a line.
    #[inline]
    pub fn unset(&mut self, offset: Offset) {
        self.0.remove(&offset);
    }

    /// The number of lines for which values are contained in this set.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return true if the values is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Construct a `Values` collection for a set of line offsets.
    ///
    /// Typically used to create a values template to be populated via a call to
    /// [`crate::request::Request::values()`].
    ///
    /// All lines are set to inactive.
    pub fn from_offsets(offsets: &[Offset]) -> Values {
        let mut values = Values::default();
        for offset in offsets {
            values.set(*offset, Value::Inactive);
        }
        values
    }

    /// An iterator to visit all values.
    pub fn iter(&self) -> Iter<'_, Offset, Value> {
        self.0.iter()
    }

    /// Return true if values contains a matching key.
    #[cfg(feature = "uapi_v1")]
    pub(crate) fn contains_key(&self, offset: &Offset) -> bool {
        self.0.contains_key(offset)
    }
}

/// The direction of a line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// The line is an input.
    Input,

    /// The line is an output.
    Output,
}
impl Default for Direction {
    fn default() -> Self {
        Self::Input
    }
}
#[cfg(feature = "uapi_v1")]
impl From<v1::LineInfoFlags> for Direction {
    fn from(flags: v1::LineInfoFlags) -> Self {
        if flags.contains(v1::LineInfoFlags::OUTPUT) {
            return Direction::Output;
        }
        Direction::Input
    }
}
#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
impl From<v2::LineFlags> for Direction {
    fn from(flags: v2::LineFlags) -> Self {
        if flags.contains(v2::LineFlags::OUTPUT) {
            return Direction::Output;
        }
        Direction::Input
    }
}

/// The bias settings for a line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bias {
    /// The line has pull-up enabled.
    PullUp,

    /// The line has pull-down enabled.
    PullDown,

    /// The line has bias disabled and will float unless externally driven.
    Disabled,
}

#[cfg(feature = "uapi_v1")]
impl TryFrom<v1::LineInfoFlags> for Bias {
    type Error = ();

    fn try_from(flags: v1::LineInfoFlags) -> Result<Self, Self::Error> {
        if flags.contains(v1::LineInfoFlags::BIAS_PULL_UP) {
            return Ok(Bias::PullUp);
        }
        if flags.contains(v1::LineInfoFlags::BIAS_PULL_DOWN) {
            return Ok(Bias::PullDown);
        }
        if flags.contains(v1::LineInfoFlags::BIAS_DISABLED) {
            return Ok(Bias::Disabled);
        }
        Err(())
    }
}
#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
impl TryFrom<v2::LineFlags> for Bias {
    type Error = ();

    fn try_from(flags: v2::LineFlags) -> Result<Self, Self::Error> {
        if flags.contains(v2::LineFlags::BIAS_PULL_UP) {
            return Ok(Bias::PullUp);
        }
        if flags.contains(v2::LineFlags::BIAS_PULL_DOWN) {
            return Ok(Bias::PullDown);
        }
        if flags.contains(v2::LineFlags::BIAS_DISABLED) {
            return Ok(Bias::Disabled);
        }
        Err(())
    }
}

/// The drive policy settings for an output line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Drive {
    /// The line is driven when both active and inactive.
    ///
    /// This is the default if drive is not specified.
    PushPull,

    /// The line is driven when low and set high impedance when high.
    OpenDrain,

    /// The line is driven when high and set high impedance when low.
    OpenSource,
}
impl Default for Drive {
    fn default() -> Self {
        Self::PushPull
    }
}
#[cfg(feature = "uapi_v1")]
impl TryFrom<v1::LineInfoFlags> for Drive {
    type Error = ();

    fn try_from(flags: v1::LineInfoFlags) -> Result<Self, Self::Error> {
        if flags.contains(v1::LineInfoFlags::OPEN_DRAIN) {
            return Ok(Drive::OpenDrain);
        }
        if flags.contains(v1::LineInfoFlags::OPEN_SOURCE) {
            return Ok(Drive::OpenSource);
        }
        if flags.contains(v1::LineInfoFlags::OUTPUT) {
            return Ok(Drive::PushPull);
        }
        Err(())
    }
}
#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
impl TryFrom<v2::LineFlags> for Drive {
    type Error = ();

    fn try_from(flags: v2::LineFlags) -> Result<Self, Self::Error> {
        if flags.contains(v2::LineFlags::OPEN_DRAIN) {
            return Ok(Drive::OpenDrain);
        }
        if flags.contains(v2::LineFlags::OPEN_SOURCE) {
            return Ok(Drive::OpenSource);
        }
        if flags.contains(v2::LineFlags::OUTPUT) {
            return Ok(Drive::PushPull);
        }
        Err(())
    }
}

/// The edge detection options for an input line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeDetection {
    /// Edge detection is only enabled on rising edges.
    ///
    /// A rising edge means a transition from an inactive state to an active state.
    RisingEdge,

    /// Edge detection is only enabled on falling edges.
    ///
    /// A falling edge means a transition from an active state to an inactive state.
    FallingEdge,

    /// Edge detection is enabled on both rising and falling edges.
    BothEdges,
}
#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
impl TryFrom<v2::LineFlags> for EdgeDetection {
    type Error = ();

    fn try_from(flags: v2::LineFlags) -> Result<Self, Self::Error> {
        if flags.contains(v2::LineFlags::EDGE_RISING | v2::LineFlags::EDGE_FALLING) {
            return Ok(EdgeDetection::BothEdges);
        }
        if flags.contains(v2::LineFlags::EDGE_RISING) {
            return Ok(EdgeDetection::RisingEdge);
        }
        if flags.contains(v2::LineFlags::EDGE_FALLING) {
            return Ok(EdgeDetection::FallingEdge);
        }
        Err(())
    }
}

/// The available clock sources for [`EdgeEvent`] timestamps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventClock {
    /// The **CLOCK_MONOTONIC** is used as the source for edge event timestamps.
    ///
    /// This is the default for ABI v2.
    Monotonic,

    /// The **CLOCK_REALTIME** is used as the source for edge event timestamps.
    Realtime,

    /// The hardware timestamp engine provides event timestamps.
    ///
    /// This source requires a Linux kernel v5.19 or later with CONFIG_HTE
    /// enabled and suitable supporting hardware.
    Hte,
}
impl Default for EventClock {
    fn default() -> Self {
        Self::Monotonic
    }
}
#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
impl From<v2::LineFlags> for EventClock {
    fn from(flags: v2::LineFlags) -> Self {
        if flags.contains(v2::LineFlags::EVENT_CLOCK_REALTIME) {
            return EventClock::Realtime;
        }
        if flags.contains(v2::LineFlags::EVENT_CLOCK_HTE) {
            return EventClock::Hte;
        }
        EventClock::Monotonic
    }
}

/// The details of an edge detected on an input line.
///
/// ABI v1 does not provide the seqno nor line_seqno fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdgeEvent {
    /// The best estimate of time of event occurrence, in nanoseconds.
    ///
    /// The interpretation of this field depends on line [`EventClock`] configuration
    /// and uAPI ABI version, and so is left raw here.
    ///
    /// **CLOCK_REALTIME** can be converted to [`Timestamp`](super::Timestamp).
    ///
    /// **CLOCK_MONOTONIC** is intended for comparing times between events and
    /// should be converted to [`Duration`].
    pub timestamp_ns: u64,

    /// The event trigger identifier.
    pub kind: EdgeKind,

    /// The offset of the line that triggered the event.
    pub offset: Offset,

    /// The sequence number for this event in the sequence of events for all
    /// the lines in this line request.
    pub seqno: u32,

    /// The sequence number for this event in the sequence of events on this
    /// particular line.
    pub line_seqno: u32,
}
#[cfg(feature = "uapi_v1")]
impl From<&v1::LineEdgeEvent> for EdgeEvent {
    fn from(le: &v1::LineEdgeEvent) -> Self {
        EdgeEvent {
            timestamp_ns: le.timestamp_ns,
            kind: EdgeKind::from(le.kind),
            // v1 doesn't provide the remaining fields...
            offset: 0,
            seqno: 0,
            line_seqno: 0,
        }
    }
}
#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
impl From<&v2::LineEdgeEvent> for EdgeEvent {
    fn from(le: &v2::LineEdgeEvent) -> Self {
        EdgeEvent {
            timestamp_ns: le.timestamp_ns,
            kind: EdgeKind::from(le.kind),
            offset: le.offset,
            seqno: le.seqno,
            line_seqno: le.line_seqno,
        }
    }
}

/// The cause of an [`EdgeEvent`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeKind {
    /// Indicates the line transitioned from inactive to active.
    Rising = 1,

    /// Indicates the line transitioned from active to inactive.
    Falling = 2,
}
impl From<uapi::LineEdgeEventKind> for EdgeKind {
    fn from(kind: uapi::LineEdgeEventKind) -> Self {
        match kind {
            uapi::LineEdgeEventKind::RisingEdge => EdgeKind::Rising,
            uapi::LineEdgeEventKind::FallingEdge => EdgeKind::Falling,
        }
    }
}

/// The details of a change to the [`Info`] for a line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InfoChangeEvent {
    /// The updated line info.
    pub info: Info,

    /// The best estimate of time of event occurrence.
    ///
    /// The **CLOCK_MONOTONIC** is used as the source for info change timestamps.
    pub timestamp_ns: u64,

    /// The trigger for the change.
    pub kind: InfoChangeKind,
}
#[cfg(feature = "uapi_v1")]
impl From<&v1::LineInfoChangeEvent> for InfoChangeEvent {
    fn from(ice: &v1::LineInfoChangeEvent) -> Self {
        InfoChangeEvent {
            info: Info::from(&ice.info),
            timestamp_ns: ice.timestamp_ns,
            kind: InfoChangeKind::from(ice.kind),
        }
    }
}
#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
impl From<&v2::LineInfoChangeEvent> for InfoChangeEvent {
    fn from(ice: &v2::LineInfoChangeEvent) -> Self {
        InfoChangeEvent {
            info: Info::from(&ice.info),
            timestamp_ns: ice.timestamp_ns,
            kind: InfoChangeKind::from(ice.kind),
        }
    }
}

/// The cause of a [`InfoChangeEvent`]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InfoChangeKind {
    /// Line has been requested.
    Requested = 1,

    /// Line has been released.
    Released = 2,

    /// Line has been reconfigured.
    Reconfigured = 3,
}
impl From<uapi::LineInfoChangeKind> for InfoChangeKind {
    fn from(kind: uapi::LineInfoChangeKind) -> Self {
        match kind {
            uapi::LineInfoChangeKind::Requested => InfoChangeKind::Requested,
            uapi::LineInfoChangeKind::Released => InfoChangeKind::Released,
            uapi::LineInfoChangeKind::Reconfigured => InfoChangeKind::Reconfigured,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod config {
        use super::*;

        #[test]
        fn default() {
            let cfg: Config = Default::default();
            assert_eq!(cfg.direction, None);
            assert!(!cfg.active_low);
            assert!(cfg.bias.is_none());
            assert!(cfg.drive.is_none());
            assert!(cfg.edge_detection.is_none());
            assert!(cfg.event_clock.is_none());
            assert!(cfg.debounce_period.is_none());
            assert!(cfg.value.is_none());
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn equivalent() {
            let mut lcfg: Config = Default::default();
            let mut rcfg: Config = Default::default();
            assert!(lcfg.equivalent(&rcfg));

            rcfg.value = Some(Value::Active);
            assert!(lcfg.equivalent(&rcfg));

            lcfg.active_low = true;
            assert!(!lcfg.equivalent(&rcfg));

            rcfg.active_low = true;
            assert!(lcfg.equivalent(&rcfg));

            lcfg.bias = Some(Bias::PullDown);
            assert!(!lcfg.equivalent(&rcfg));

            rcfg.bias = Some(Bias::PullDown);
            assert!(lcfg.equivalent(&rcfg));

            lcfg.drive = Some(Drive::OpenDrain);
            assert!(!lcfg.equivalent(&rcfg));

            rcfg.drive = Some(Drive::OpenDrain);
            assert!(lcfg.equivalent(&rcfg));

            lcfg.edge_detection = Some(EdgeDetection::RisingEdge);
            assert!(!lcfg.equivalent(&rcfg));

            rcfg.edge_detection = Some(EdgeDetection::RisingEdge);
            assert!(lcfg.equivalent(&rcfg));

            lcfg.event_clock = Some(EventClock::Realtime);
            assert!(!lcfg.equivalent(&rcfg));

            rcfg.event_clock = Some(EventClock::Realtime);
            assert!(lcfg.equivalent(&rcfg));

            lcfg.debounce_period = Some(Duration::from_millis(5));
            assert!(!lcfg.equivalent(&rcfg));

            rcfg.debounce_period = Some(Duration::from_millis(5));
            assert!(lcfg.equivalent(&rcfg));
        }

        #[test]
        fn value() {
            let mut cfg: Config = Default::default();
            assert_eq!(cfg.value(), Value::Inactive);

            cfg.value = Some(Value::Active);
            assert_eq!(cfg.value(), Value::Active);

            cfg.value = Some(Value::Inactive);
            assert_eq!(cfg.value(), Value::Inactive);
        }

        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn v2_line_flags_from_config() {
            let cfg = Config {
                direction: Some(Direction::Input),
                active_low: false,
                bias: Some(Bias::Disabled),
                drive: Some(Drive::OpenDrain), // ignored for input
                edge_detection: Some(EdgeDetection::BothEdges),
                event_clock: Some(EventClock::Realtime),
                debounce_period: None,
                value: None,
            };
            let flags = v2::LineFlags::from(&cfg);
            assert!(flags.contains(v2::LineFlags::INPUT));
            assert!(!flags.contains(v2::LineFlags::OUTPUT));
            assert!(!flags.contains(v2::LineFlags::ACTIVE_LOW));
            assert!(flags.contains(v2::LineFlags::BIAS_DISABLED));
            assert!(!flags.contains(v2::LineFlags::BIAS_PULL_UP));
            assert!(!flags.contains(v2::LineFlags::BIAS_PULL_DOWN));
            assert!(flags.contains(v2::LineFlags::EDGE_RISING));
            assert!(flags.contains(v2::LineFlags::EDGE_FALLING));
            assert!(!flags.contains(v2::LineFlags::OPEN_DRAIN));
            assert!(!flags.contains(v2::LineFlags::OPEN_SOURCE));
            assert!(flags.contains(v2::LineFlags::EVENT_CLOCK_REALTIME));

            let cfg = Config {
                direction: Some(Direction::Input),
                active_low: true,
                bias: Some(Bias::PullUp),
                drive: Some(Drive::OpenSource), // ignored for input
                edge_detection: None,
                event_clock: Some(EventClock::Hte), // ignored for no edges
                debounce_period: None,
                value: None,
            };
            let flags = v2::LineFlags::from(&cfg);
            assert!(flags.contains(v2::LineFlags::INPUT));
            assert!(!flags.contains(v2::LineFlags::OUTPUT));
            assert!(flags.contains(v2::LineFlags::ACTIVE_LOW));
            assert!(!flags.contains(v2::LineFlags::BIAS_DISABLED));
            assert!(flags.contains(v2::LineFlags::BIAS_PULL_UP));
            assert!(!flags.contains(v2::LineFlags::BIAS_PULL_DOWN));
            assert!(!flags.contains(v2::LineFlags::EDGE_RISING));
            assert!(!flags.contains(v2::LineFlags::EDGE_FALLING));
            assert!(!flags.contains(v2::LineFlags::OPEN_DRAIN));
            assert!(!flags.contains(v2::LineFlags::OPEN_SOURCE));
            assert!(!flags.contains(v2::LineFlags::EVENT_CLOCK_HTE));

            let cfg = Config {
                direction: Some(Direction::Output),
                active_low: false,
                bias: Some(Bias::PullDown),
                drive: Some(Drive::OpenSource),
                edge_detection: Some(EdgeDetection::BothEdges), // ignored for output
                event_clock: Some(EventClock::Realtime),        // ignored for output
                debounce_period: None,
                value: None,
            };
            let flags = v2::LineFlags::from(&cfg);
            assert!(!flags.contains(v2::LineFlags::INPUT));
            assert!(flags.contains(v2::LineFlags::OUTPUT));
            assert!(!flags.contains(v2::LineFlags::ACTIVE_LOW));
            assert!(!flags.contains(v2::LineFlags::BIAS_DISABLED));
            assert!(!flags.contains(v2::LineFlags::BIAS_PULL_UP));
            assert!(flags.contains(v2::LineFlags::BIAS_PULL_DOWN));
            assert!(!flags.contains(v2::LineFlags::EDGE_RISING));
            assert!(!flags.contains(v2::LineFlags::EDGE_FALLING));
            assert!(!flags.contains(v2::LineFlags::OPEN_DRAIN));
            assert!(flags.contains(v2::LineFlags::OPEN_SOURCE));
            assert!(!flags.contains(v2::LineFlags::EVENT_CLOCK_REALTIME));
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn v1_event_request_flags_from_config() {
            let mut cfg = Config {
                direction: Some(Direction::Input),
                active_low: false,
                bias: Some(Bias::Disabled),
                drive: Some(Drive::OpenDrain),
                edge_detection: Some(EdgeDetection::BothEdges),
                event_clock: Some(EventClock::Realtime),
                debounce_period: None,
                value: None,
            };
            let flags = v1::EventRequestFlags::from(&cfg);
            assert!(flags.contains(v1::EventRequestFlags::RISING_EDGE));
            assert!(flags.contains(v1::EventRequestFlags::FALLING_EDGE));
            assert!(flags.contains(v1::EventRequestFlags::BOTH_EDGES));

            cfg.edge_detection = Some(EdgeDetection::RisingEdge);
            let flags = v1::EventRequestFlags::from(&cfg);
            assert!(flags.contains(v1::EventRequestFlags::RISING_EDGE));
            assert!(!flags.contains(v1::EventRequestFlags::FALLING_EDGE));
            assert!(!flags.contains(v1::EventRequestFlags::BOTH_EDGES));

            cfg.edge_detection = Some(EdgeDetection::FallingEdge);
            let flags = v1::EventRequestFlags::from(&cfg);
            assert!(!flags.contains(v1::EventRequestFlags::RISING_EDGE));
            assert!(flags.contains(v1::EventRequestFlags::FALLING_EDGE));
            assert!(!flags.contains(v1::EventRequestFlags::BOTH_EDGES));

            cfg.edge_detection = None;
            let flags = v1::EventRequestFlags::from(&cfg);
            assert!(!flags.contains(v1::EventRequestFlags::RISING_EDGE));
            assert!(!flags.contains(v1::EventRequestFlags::FALLING_EDGE));
            assert!(!flags.contains(v1::EventRequestFlags::BOTH_EDGES));
        }
        #[test]
        #[cfg(feature = "uapi_v1")]
        fn v1_handle_request_flags_from_config() {
            let cfg = Config {
                direction: Some(Direction::Input),
                active_low: false,
                bias: Some(Bias::Disabled),
                drive: Some(Drive::OpenDrain), // ignored for input
                edge_detection: Some(EdgeDetection::BothEdges),
                event_clock: None,
                debounce_period: None,
                value: None,
            };
            let flags = v1::HandleRequestFlags::from(&cfg);
            assert!(flags.contains(v1::HandleRequestFlags::INPUT));
            assert!(!flags.contains(v1::HandleRequestFlags::OUTPUT));
            assert!(!flags.contains(v1::HandleRequestFlags::ACTIVE_LOW));
            assert!(flags.contains(v1::HandleRequestFlags::BIAS_DISABLED));
            assert!(!flags.contains(v1::HandleRequestFlags::BIAS_PULL_UP));
            assert!(!flags.contains(v1::HandleRequestFlags::BIAS_PULL_DOWN));
            assert!(!flags.contains(v1::HandleRequestFlags::OPEN_DRAIN));
            assert!(!flags.contains(v1::HandleRequestFlags::OPEN_SOURCE));

            let cfg = Config {
                direction: Some(Direction::Input),
                active_low: true,
                bias: Some(Bias::PullUp),
                drive: Some(Drive::OpenSource), // ignored for input
                edge_detection: None,
                event_clock: None,
                debounce_period: None,
                value: None,
            };
            let flags = v1::HandleRequestFlags::from(&cfg);
            assert!(flags.contains(v1::HandleRequestFlags::INPUT));
            assert!(!flags.contains(v1::HandleRequestFlags::OUTPUT));
            assert!(flags.contains(v1::HandleRequestFlags::ACTIVE_LOW));
            assert!(!flags.contains(v1::HandleRequestFlags::BIAS_DISABLED));
            assert!(flags.contains(v1::HandleRequestFlags::BIAS_PULL_UP));
            assert!(!flags.contains(v1::HandleRequestFlags::BIAS_PULL_DOWN));
            assert!(!flags.contains(v1::HandleRequestFlags::OPEN_DRAIN));
            assert!(!flags.contains(v1::HandleRequestFlags::OPEN_SOURCE));

            let cfg = Config {
                direction: Some(Direction::Output),
                active_low: false,
                bias: Some(Bias::PullDown),
                drive: Some(Drive::OpenSource),
                edge_detection: Some(EdgeDetection::BothEdges), // ignored for output
                event_clock: None,
                debounce_period: None,
                value: None,
            };
            let flags = v1::HandleRequestFlags::from(&cfg);
            assert!(!flags.contains(v1::HandleRequestFlags::INPUT));
            assert!(flags.contains(v1::HandleRequestFlags::OUTPUT));
            assert!(!flags.contains(v1::HandleRequestFlags::ACTIVE_LOW));
            assert!(!flags.contains(v1::HandleRequestFlags::BIAS_DISABLED));
            assert!(!flags.contains(v1::HandleRequestFlags::BIAS_PULL_UP));
            assert!(flags.contains(v1::HandleRequestFlags::BIAS_PULL_DOWN));
            assert!(!flags.contains(v1::HandleRequestFlags::OPEN_DRAIN));
            assert!(flags.contains(v1::HandleRequestFlags::OPEN_SOURCE));
        }
    }
    mod info {
        use super::*;

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn info_from_v1_line_info() {
            let v1info: v1::LineInfo = Default::default();
            let info = Info::from(&v1info);
            assert_eq!(info.offset, 0);
            assert!(info.name.is_empty());
            assert!(info.consumer.is_empty());
            assert!(!info.used);
            assert!(!info.active_low);
            assert_eq!(info.direction, Direction::Input);
            assert!(info.bias.is_none());
            assert!(info.drive.is_none());
            assert!(info.edge_detection.is_none());
            assert!(info.event_clock.is_none());
            assert!(info.debounce_period.is_none());

            let v1info = v1::LineInfo {
                offset: 32,
                flags: v1::LineInfoFlags::USED
                    | v1::LineInfoFlags::ACTIVE_LOW
                    | v1::LineInfoFlags::OUTPUT
                    | v1::LineInfoFlags::BIAS_PULL_DOWN,
                name: "banana".into(),
                consumer: "jam".into(),
            };
            let info = Info::from(&v1info);
            assert_eq!(info.offset, 32);
            assert_eq!(info.name, "banana");
            assert_eq!(info.consumer, "jam");
            assert!(info.used);
            assert!(info.active_low);
            assert_eq!(info.direction, Direction::Output);
            assert_eq!(info.bias, Some(Bias::PullDown));
            assert_eq!(info.drive, Some(Drive::PushPull));
            assert!(info.edge_detection.is_none());
            assert!(info.event_clock.is_none());
            assert!(info.debounce_period.is_none());

            let v1info = v1::LineInfo {
                offset: 32,
                flags: v1::LineInfoFlags::USED
                    | v1::LineInfoFlags::OUTPUT
                    | v1::LineInfoFlags::OPEN_DRAIN
                    | v1::LineInfoFlags::BIAS_DISABLED,
                name: "banana".into(),
                consumer: "jam".into(),
            };
            let info = Info::from(&v1info);
            assert_eq!(info.offset, 32);
            assert_eq!(info.name, "banana");
            assert_eq!(info.consumer, "jam");
            assert!(info.used);
            assert!(!info.active_low);
            assert_eq!(info.direction, Direction::Output);
            assert_eq!(info.bias, Some(Bias::Disabled));
            assert_eq!(info.drive, Some(Drive::OpenDrain));
            assert!(info.edge_detection.is_none());
            assert!(info.event_clock.is_none());
            assert!(info.debounce_period.is_none());
        }
        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn info_from_v2_line_info() {
            let v2info: v2::LineInfo = Default::default();
            let info = Info::from(&v2info);
            assert_eq!(info.offset, 0);
            assert!(info.name.is_empty());
            assert!(info.consumer.is_empty());
            assert!(!info.used);
            assert!(!info.active_low);
            assert_eq!(info.direction, Direction::Input);
            assert!(info.bias.is_none());
            assert!(info.drive.is_none());
            assert!(info.edge_detection.is_none());
            assert!(info.event_clock.is_none());
            assert!(info.debounce_period.is_none());

            let v2info = v2::LineInfo {
                offset: 32,
                flags: v2::LineFlags::USED
                    | v2::LineFlags::ACTIVE_LOW
                    | v2::LineFlags::OUTPUT
                    | v2::LineFlags::BIAS_PULL_DOWN,
                name: "banana".into(),
                consumer: "jam".into(),
                num_attrs: 0,
                attrs: Default::default(),
                padding: Default::default(),
            };
            let info = Info::from(&v2info);
            assert_eq!(info.offset, 32);
            assert_eq!(info.name, "banana");
            assert_eq!(info.consumer, "jam");
            assert!(info.used);
            assert!(info.active_low);
            assert_eq!(info.direction, Direction::Output);
            assert_eq!(info.bias, Some(Bias::PullDown));
            assert_eq!(info.drive, Some(Drive::PushPull));
            assert!(info.edge_detection.is_none());
            assert!(info.event_clock.is_none());
            assert!(info.debounce_period.is_none());

            let v2info = v2::LineInfo {
                offset: 32,
                flags: v2::LineFlags::USED
                    | v2::LineFlags::OUTPUT
                    | v2::LineFlags::OPEN_DRAIN
                    | v2::LineFlags::BIAS_DISABLED,
                name: "banana".into(),
                consumer: "jam".into(),
                num_attrs: 0,
                attrs: Default::default(),
                padding: Default::default(),
            };
            let info = Info::from(&v2info);
            assert_eq!(info.offset, 32);
            assert_eq!(info.name, "banana");
            assert_eq!(info.consumer, "jam");
            assert!(info.used);
            assert!(!info.active_low);
            assert_eq!(info.direction, Direction::Output);
            assert_eq!(info.bias, Some(Bias::Disabled));
            assert_eq!(info.drive, Some(Drive::OpenDrain));
            assert!(info.edge_detection.is_none());
            assert!(info.event_clock.is_none());
            assert!(info.debounce_period.is_none());

            let v2info = v2::LineInfo {
                offset: 32,
                flags: v2::LineFlags::USED
                    | v2::LineFlags::INPUT
                    | v2::LineFlags::EDGE_RISING
                    | v2::LineFlags::BIAS_PULL_DOWN,
                name: "banana".into(),
                consumer: "jam".into(),
                num_attrs: 0,
                attrs: Default::default(),
                padding: Default::default(),
            };
            let info = Info::from(&v2info);
            assert_eq!(info.offset, 32);
            assert_eq!(info.name, "banana");
            assert_eq!(info.consumer, "jam");
            assert!(info.used);
            assert!(!info.active_low);
            assert_eq!(info.direction, Direction::Input);
            assert_eq!(info.bias, Some(Bias::PullDown));
            assert!(info.drive.is_none());
            assert_eq!(info.edge_detection, Some(EdgeDetection::RisingEdge));
            assert_eq!(info.event_clock, Some(EventClock::Monotonic));
            assert!(info.debounce_period.is_none());
        }
    }

    mod value {
        use super::*;

        #[test]
        fn default() {
            let v = Value::default();
            assert_eq!(v, Value::Inactive);
        }

        #[test]
        fn not() {
            assert_eq!(Value::Active.not(), Value::Inactive);
            assert_eq!(Value::Inactive.not(), Value::Active);
        }

        #[test]
        fn from_bool() {
            assert_eq!(Value::from(true), Value::Active);
            assert_eq!(Value::from(false), Value::Inactive);
        }

        #[test]
        fn into_bool() {
            let b: bool = Value::Active.into();
            assert!(b);
            let b: bool = Value::Inactive.into();
            assert!(!b);
        }

        #[test]
        fn from_u8() {
            assert_eq!(Value::from(1), Value::Active);
            assert_eq!(Value::from(2), Value::Active);
            assert_eq!(Value::from(0), Value::Inactive);
        }

        #[test]
        fn into_u8() {
            let u: u8 = Value::Active.into();
            assert_eq!(u, 1);
            let u: u8 = Value::Inactive.into();
            assert_eq!(u, 0);
        }
    }

    mod values {
        use super::*;

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn from_v1() {
            let offsets = Vec::from([1, 5, 3, 8]);
            let src = v1::LineValues::from_slice(&[1, 1, 0, 1]);
            let mut dst = Values::default();
            dst.set(4, Value::Active);
            dst.set(7, Value::Inactive);
            dst.overlay_from_v1(&offsets, &src);
            assert_eq!(dst.get(1), Some(Value::Active));
            assert_eq!(dst.get(2), None);
            assert_eq!(dst.get(3), Some(Value::Inactive));
            assert_eq!(dst.get(4), Some(Value::Active));
            assert_eq!(dst.get(5), Some(Value::Active));
            assert_eq!(dst.get(6), None);
            assert_eq!(dst.get(7), Some(Value::Inactive));
            assert_eq!(dst.get(8), Some(Value::Active));
        }

        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn from_v2() {
            let offsets = Vec::from([1, 5, 3, 8]);
            let mut src = v2::LineValues::default();
            src.set(0, true);
            src.set(1, true);
            src.set(2, false);
            src.set(3, true);
            let mut dst = Values::default();
            dst.set(4, Value::Active);
            dst.set(7, Value::Inactive);
            dst.overlay_from_v2(&offsets, &src);
            assert_eq!(dst.get(1), Some(Value::Active));
            assert_eq!(dst.get(2), None);
            assert_eq!(dst.get(3), Some(Value::Inactive));
            assert_eq!(dst.get(4), Some(Value::Active));
            assert_eq!(dst.get(5), Some(Value::Active));
            assert_eq!(dst.get(6), None);
            assert_eq!(dst.get(7), Some(Value::Inactive));
            assert_eq!(dst.get(8), Some(Value::Active));
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn to_v1() {
            let offsets = Vec::from([1, 5, 3, 8]);
            let mut src = Values::default();
            src.set(1, Value::Active);
            src.set(3, Value::Inactive);
            src.set(7, Value::Active); // should be ignored
            src.set(8, Value::Active);
            let dst = src.to_v1(&offsets);
            assert_eq!(dst.get(0), 1); // 1
            assert_eq!(dst.get(1), 0); // 5
            assert_eq!(dst.get(2), 0); // 3
            assert_eq!(dst.get(3), 1); // 8
        }

        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn to_v2() {
            let offsets = Vec::from([1, 5, 3, 8]);
            let mut src = Values::default();
            src.set(1, Value::Active);
            src.set(3, Value::Inactive);
            src.set(7, Value::Active); // should be ignored
            src.set(8, Value::Active);
            let dst = src.to_v2(&offsets);
            assert!(dst.get(0).unwrap()); // 1
            assert!(dst.get(1).is_none()); // 5
            assert!(!dst.get(2).unwrap()); // 3
            assert!(dst.get(3).unwrap()); // 8
            assert_eq!(dst.mask.into_value(), 0b1101); // only 3 entries set
        }

        #[test]
        fn from_offsets() {
            let values = Values::from_offsets(&[1, 6, 3, 9]);
            assert_eq!(values.get(0), None);
            assert_eq!(values.get(1), Some(Value::Inactive));
            assert_eq!(values.get(2), None);
            assert_eq!(values.get(3), Some(Value::Inactive));
            assert_eq!(values.get(4), None);
            assert_eq!(values.get(5), None);
            assert_eq!(values.get(6), Some(Value::Inactive));
            assert_eq!(values.get(7), None);
            assert_eq!(values.get(8), None);
            assert_eq!(values.get(9), Some(Value::Inactive));
        }

        #[test]
        fn get() {
            let mut vv = Values::from_offsets(&[1, 2, 3]);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));

            vv.set(2, Value::Active);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Active));
            assert_eq!(vv.get(3), Some(Value::Inactive));
        }

        #[test]
        fn set() {
            let mut vv = Values::from_offsets(&[1, 2, 3]);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));

            vv.set(2, Value::Active);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Active));
            assert_eq!(vv.get(3), Some(Value::Inactive));

            vv.set(2, Value::Inactive);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));
        }

        #[test]
        fn unset() {
            let mut vv = Values::from_offsets(&[1, 2, 3]);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));
            vv.unset(2);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), None);
            assert_eq!(vv.get(3), Some(Value::Inactive));
        }

        #[test]
        fn toggle() {
            let mut vv = Values::from_offsets(&[1, 2, 3]);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));
            vv.toggle(1);
            assert_eq!(vv.get(1), Some(Value::Active));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));
            vv.toggle(2);
            assert_eq!(vv.get(1), Some(Value::Active));
            assert_eq!(vv.get(2), Some(Value::Active));
            assert_eq!(vv.get(3), Some(Value::Inactive));
            vv.toggle(1);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Active));
            assert_eq!(vv.get(3), Some(Value::Inactive));
            vv.toggle(3);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Active));
            assert_eq!(vv.get(3), Some(Value::Active));
        }

        #[test]
        fn len() {
            assert_eq!(Values::default().len(), 0);
            assert_eq!(Values::from_offsets(&[1, 2, 3]).len(), 3);
        }

        #[test]
        fn is_empty() {
            assert!(Values::default().is_empty());
            assert!(!Values::from_offsets(&[1, 2, 3]).is_empty());
        }

        #[test]
        fn iter() {
            let mut vv = Values::from_offsets(&[1, 2, 3]);
            vv.set(2, Value::Active);
            let mut i = vv.iter();
            // assumes keys returned in order...
            assert_eq!(i.next(), Some((&1, &Value::Inactive)));
            assert_eq!(i.next(), Some((&2, &Value::Active)));
            assert_eq!(i.next(), Some((&3, &Value::Inactive)));
            assert_eq!(i.next(), None);
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn contains_key() {
            let vv = Values::from_offsets(&[1, 2, 3]);
            assert!(!vv.contains_key(&0));
            assert!(vv.contains_key(&1));
            assert!(vv.contains_key(&2));
            assert!(vv.contains_key(&3));
            assert!(!vv.contains_key(&4));
        }
    }

    mod direction {
        use super::*;

        #[test]
        fn default() {
            assert_eq!(Direction::default(), Direction::Input);
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn from_v1_line_info_flags() {
            assert_eq!(
                Direction::from(v1::LineInfoFlags::OUTPUT),
                Direction::Output
            );
            assert_eq!(
                Direction::from(v1::LineInfoFlags::ACTIVE_LOW),
                Direction::Input
            );
        }

        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn from_v2_line_flags() {
            assert_eq!(Direction::from(v2::LineFlags::OUTPUT), Direction::Output);
            assert_eq!(Direction::from(v2::LineFlags::INPUT), Direction::Input);
        }
    }

    mod bias {
        use super::*;

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn try_from_v1_line_info_flags() {
            assert_eq!(Bias::try_from(v1::LineInfoFlags::ACTIVE_LOW), Err(()));
            assert_eq!(
                Bias::try_from(v1::LineInfoFlags::BIAS_PULL_DOWN),
                Ok(Bias::PullDown)
            );
            assert_eq!(
                Bias::try_from(v1::LineInfoFlags::BIAS_PULL_UP),
                Ok(Bias::PullUp)
            );
            assert_eq!(
                Bias::try_from(v1::LineInfoFlags::BIAS_DISABLED),
                Ok(Bias::Disabled)
            );
        }

        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn from_v2_line_flags() {
            assert_eq!(Bias::try_from(v2::LineFlags::INPUT), Err(()));
            assert_eq!(
                Bias::try_from(v2::LineFlags::BIAS_PULL_DOWN),
                Ok(Bias::PullDown)
            );
            assert_eq!(
                Bias::try_from(v2::LineFlags::BIAS_PULL_UP),
                Ok(Bias::PullUp)
            );
            assert_eq!(
                Bias::try_from(v2::LineFlags::BIAS_DISABLED),
                Ok(Bias::Disabled)
            );
        }
    }

    mod drive {
        use super::*;

        #[test]
        fn default() {
            assert_eq!(Drive::default(), Drive::PushPull);
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn try_from_v1_line_info_flags() {
            assert_eq!(Drive::try_from(v1::LineInfoFlags::ACTIVE_LOW), Err(()));
            assert_eq!(
                Drive::try_from(v1::LineInfoFlags::OUTPUT),
                Ok(Drive::PushPull)
            );
            assert_eq!(
                Drive::try_from(v1::LineInfoFlags::OUTPUT | v1::LineInfoFlags::OPEN_DRAIN),
                Ok(Drive::OpenDrain)
            );
            assert_eq!(
                Drive::try_from(v1::LineInfoFlags::OUTPUT | v1::LineInfoFlags::OPEN_SOURCE),
                Ok(Drive::OpenSource)
            );
        }

        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn try_from_v2_line_flags() {
            assert_eq!(Drive::try_from(v2::LineFlags::INPUT), Err(()));
            assert_eq!(Drive::try_from(v2::LineFlags::OUTPUT), Ok(Drive::PushPull));
            assert_eq!(
                Drive::try_from(v2::LineFlags::OUTPUT | v2::LineFlags::OPEN_DRAIN),
                Ok(Drive::OpenDrain)
            );
            assert_eq!(
                Drive::try_from(v2::LineFlags::OUTPUT | v2::LineFlags::OPEN_SOURCE),
                Ok(Drive::OpenSource)
            );
        }
    }

    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    mod edge_detection {
        use super::{v2, EdgeDetection};

        #[test]
        fn try_from_v2_line_flags() {
            assert_eq!(EdgeDetection::try_from(v2::LineFlags::INPUT), Err(()));
            assert_eq!(
                EdgeDetection::try_from(v2::LineFlags::EDGE_RISING),
                Ok(EdgeDetection::RisingEdge)
            );
            assert_eq!(
                EdgeDetection::try_from(v2::LineFlags::EDGE_FALLING),
                Ok(EdgeDetection::FallingEdge)
            );
            assert_eq!(
                EdgeDetection::try_from(v2::LineFlags::EDGE_RISING | v2::LineFlags::EDGE_FALLING),
                Ok(EdgeDetection::BothEdges)
            );
        }
    }

    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    mod event_clock {
        use super::{v2, EventClock};

        #[test]
        fn default() {
            assert_eq!(EventClock::default(), EventClock::Monotonic);
        }

        #[test]
        fn from_v2_line_flags() {
            assert_eq!(
                EventClock::from(v2::LineFlags::INPUT),
                EventClock::Monotonic
            );
            assert_eq!(
                EventClock::from(v2::LineFlags::EVENT_CLOCK_REALTIME),
                EventClock::Realtime
            );
        }
    }

    mod edge_event {
        use super::*;

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn from_v1() {
            let v1event = v1::LineEdgeEvent {
                timestamp_ns: 1234,
                kind: gpiocdev_uapi::v1::LineEdgeEventKind::FallingEdge,
            };
            let ee = EdgeEvent::from(&v1event);
            assert_eq!(ee.timestamp_ns, 1234);
            assert_eq!(ee.kind, EdgeKind::Falling);
            assert_eq!(ee.offset, 0);
            assert_eq!(ee.seqno, 0);
            assert_eq!(ee.line_seqno, 0);
        }

        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn from_v2() {
            let v2event = v2::LineEdgeEvent {
                timestamp_ns: 1234,
                kind: gpiocdev_uapi::v2::LineEdgeEventKind::RisingEdge,
                offset: 23,
                seqno: 2,
                line_seqno: 1,
                padding: Default::default(),
            };
            let ee = EdgeEvent::from(&v2event);
            assert_eq!(ee.timestamp_ns, 1234);
            assert_eq!(ee.kind, EdgeKind::Rising);
            assert_eq!(ee.offset, 23);
            assert_eq!(ee.seqno, 2);
            assert_eq!(ee.line_seqno, 1);
        }
    }

    mod info_change_event {
        use super::*;

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn from_v1() {
            let v1event = v1::LineInfoChangeEvent {
                timestamp_ns: 1234,
                kind: gpiocdev_uapi::v1::LineInfoChangeKind::Reconfigured,
                info: v1::LineInfo {
                    offset: 32,
                    flags: v1::LineInfoFlags::OPEN_DRAIN,
                    name: Default::default(),
                    consumer: Default::default(),
                },
                padding: Default::default(),
            };
            let ee = InfoChangeEvent::from(&v1event);
            assert_eq!(ee.timestamp_ns, 1234);
            assert_eq!(ee.kind, InfoChangeKind::Reconfigured);
            assert_eq!(ee.info.offset, 32);
            assert_eq!(ee.info.drive, Some(Drive::OpenDrain));
        }

        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn from_v2() {
            let v2event = v2::LineInfoChangeEvent {
                timestamp_ns: 1234,
                kind: gpiocdev_uapi::v1::LineInfoChangeKind::Reconfigured,
                info: v2::LineInfo {
                    offset: 32,
                    flags: v2::LineFlags::OPEN_DRAIN,
                    name: Default::default(),
                    consumer: Default::default(),
                    num_attrs: 0,
                    attrs: Default::default(),
                    padding: Default::default(),
                },
                padding: Default::default(),
            };
            let ee = InfoChangeEvent::from(&v2event);
            assert_eq!(ee.timestamp_ns, 1234);
            assert_eq!(ee.kind, InfoChangeKind::Reconfigured);
            assert_eq!(ee.info.offset, 32);
            assert_eq!(ee.info.drive, Some(Drive::OpenDrain));
        }
    }
}
