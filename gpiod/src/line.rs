// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::{Name, Timestamp};
#[cfg(all(feature = "uapi_v1", not(feature = "uapi_v2")))]
use gpiod_uapi::v1 as uv;
#[cfg(feature = "uapi_v1")]
use gpiod_uapi::v1;
#[cfg(feature = "uapi_v2")]
use gpiod_uapi::{v2, v2 as uv};
use nohash_hasher::IntMap;
use std::time::Duration;

/// The configuration settings for a single line.
///
// Note it does not contain the offset to allow it to be applied to multiple lines.
#[derive(Clone, Debug)]
pub struct Config {
    /// The direction setting for the line.
    pub direction: Direction,
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
    pub(crate) fn value(&self) -> Value {
        match self.value {
            None => Value::Inactive,
            Some(x) => x,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            direction: Direction::Input,
            active_low: false,
            bias: None,
            drive: None,
            edge_detection: None,
            event_clock: None,
            debounce_period: None,
            value: None,
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
            Direction::Input => flags.set(v2::LineFlags::INPUT, true),
            Direction::Output => flags.set(v2::LineFlags::OUTPUT, true),
        };
        if cfg.direction == Direction::Output {
            match cfg.drive {
                None => {}
                Some(Drive::PushPull) => {}
                Some(Drive::OpenDrain) => flags.set(v2::LineFlags::OPEN_DRAIN, true),
                Some(Drive::OpenSource) => flags.set(v2::LineFlags::OPEN_SOURCE, true),
            };
        }
        if cfg.direction == Direction::Input {
            match cfg.edge_detection {
                None => {}
                Some(EdgeDetection::RisingEdge) => flags.set(v2::LineFlags::EDGE_RISING, true),
                Some(EdgeDetection::FallingEdge) => flags.set(v2::LineFlags::EDGE_FALLING, true),
                Some(EdgeDetection::BothEdges) => flags.set(
                    v2::LineFlags::EDGE_RISING | v2::LineFlags::EDGE_FALLING,
                    true,
                ),
            };
        }
        if cfg.edge_detection.is_some() {
            match cfg.event_clock {
                None => {}
                Some(EventClock::Monotonic) => {}
                Some(EventClock::Realtime) => flags.set(v2::LineFlags::EVENT_CLOCK_REALTIME, true),
            };
        }
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
            Direction::Input => flags.set(v1::HandleRequestFlags::INPUT, true),
            Direction::Output => flags.set(v1::HandleRequestFlags::OUTPUT, true),
        };
        if cfg.active_low {
            flags.set(v1::HandleRequestFlags::ACTIVE_LOW, true);
        }
        match cfg.drive {
            None => {}
            Some(Drive::PushPull) => {}
            Some(Drive::OpenDrain) => flags.set(v1::HandleRequestFlags::OPEN_DRAIN, true),
            Some(Drive::OpenSource) => flags.set(v1::HandleRequestFlags::OPEN_SOURCE, true),
        };
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
#[derive(Default, Debug)]
pub struct Info {
    /// The line offset on the GPIO chip.
    pub offset: Offset,
    /// The name of this GPIO line, such as the output pin of the line on
    /// the chip, a rail or a pin header name on a board, as specified by the
    /// GPIO chip.
    ///
    /// May be empty.
    pub name: Name,
    /// A functional name for the consumer of this GPIO line as set
    /// by whatever is using it.
    ///
    /// May be empty if not set by the user or the line is unused.
    pub consumer: Name,
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
    pub edge: Option<EdgeDetection>,
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
            name: Name::from(&li.name),
            consumer: Name::from(&li.consumer),
            used: li.flags & v1::LineInfoFlags::USED == v1::LineInfoFlags::USED,
            active_low: li.flags & v1::LineInfoFlags::ACTIVE_LOW == v1::LineInfoFlags::ACTIVE_LOW,
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
        Info {
            offset: li.offset,
            name: Name::from(&li.name),
            consumer: Name::from(&li.consumer),
            used: li.flags & v2::LineFlags::USED == v2::LineFlags::USED,
            active_low: li.flags & v2::LineFlags::ACTIVE_LOW == v2::LineFlags::ACTIVE_LOW,
            direction: Direction::from(li.flags),
            bias: Bias::try_from(li.flags).ok(),
            drive: Drive::try_from(li.flags).ok(),
            edge: EdgeDetection::try_from(li.flags).ok(),
            event_clock: EventClock::try_from(li.flags).ok(),
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
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    /// The line is inactive.
    Inactive,
    /// The line is active.
    Active,
}
impl Value {
    pub fn toggle(&self) -> Value {
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
#[derive(Debug, Default)]
pub struct Values(IntMap<Offset, Value>);
impl Values {
    /// overlays the values from src over the values in the dst.
    #[cfg(all(feature = "uapi_v1", not(feature = "uapi_v2")))]
    pub(crate) fn from_uapi(&mut self, offsets: &[Offset], src: &uv::LineValues) {
        self.from_v1(offsets, src)
    }
    #[cfg(not(feature = "uapi_v1"))]
    pub(crate) fn from_uapi(&mut self, offsets: &[Offset], src: &uv::LineValues) {
        self.from_v2(offsets, src)
    }
    /// overlays the values from src over the values in the dst.
    #[cfg(feature = "uapi_v1")]
    pub(crate) fn from_v1(&mut self, offsets: &[Offset], src: &v1::LineValues) {
        for (idx, offset) in offsets.iter().enumerate() {
            self.0.insert(*offset, src.get(idx).into());
        }
    }
    /// overlays the values from src over the values in the dst.
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    pub(crate) fn from_v2(&mut self, offsets: &[Offset], src: &v2::LineValues) {
        for (idx, offset) in offsets.iter().enumerate() {
            if let Some(val) = src.get(idx) {
                self.0.insert(*offset, val.into());
            }
        }
    }
    #[cfg(all(feature = "uapi_v1", not(feature = "uapi_v2")))]
    pub(crate) fn to_uapi(&self, offsets: &[Offset]) -> uv::LineValues {
        self.to_v1(offsets)
    }
    #[cfg(not(feature = "uapi_v1"))]
    pub(crate) fn to_uapi(&self, offsets: &[Offset]) -> uv::LineValues {
        self.to_v2(offsets)
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
            if let Some(val) = self.0.get(offset) {
                dst.set(idx, (*val).into());
            }
        }
        dst
    }

    /// Get the value of a line.
    #[inline]
    pub fn get(&mut self, offset: Offset) -> Option<Value> {
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
        self.0.insert(offset, val.toggle());
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

    /// Construct a `Values` collection for a set of lines.
    ///
    /// Typically used to create a values template to be populated via a call to
    /// [`crate::request::Request::values()`].
    ///
    /// All lines are set to inactive.
    pub fn from_lines(offsets: &[Offset]) -> Values {
        let mut values = Values::default();
        for offset in offsets {
            values.set(*offset, Value::Inactive);
        }
        values
    }
}

/// The direction of a line.
#[derive(Copy, Clone, Debug, PartialEq)]
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
#[derive(Copy, Clone, Debug, PartialEq)]
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
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Drive {
    /// The line is driven when both active and inactive.
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
#[derive(Copy, Clone, Debug, PartialEq)]
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
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum EventClock {
    /// The **CLOCK_MONOTONIC** is used as the source for edge event timestamps.
    ///
    /// This is the default for ABI v2.
    Monotonic,
    /// The **CLOCK_REALTIME** is used as the source for edge event timestamps.
    Realtime,
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
        EventClock::Monotonic
    }
}

/// The details of an edge detected on an input line.
///
/// ABI v1 does not provide the seqno nor line_seqno fields.
#[derive(Debug)]
pub struct EdgeEvent {
    /// The best estimate of time of event occurrence, in nanoseconds.
    ///
    /// The interpretation of this field depends on line [`EventClock`] configuration
    /// and uAPI ABI version, and so is left raw here.
    ///
    /// **CLOCK_MONOTONIC** can be converted to [`Timestamp`].
    ///
    /// **CLOCK_REALTIME** is intended for comparing times between events and
    /// should be converted to [`Duration`].
    pub timestamp_ns: u64,
    /// The event trigger identifier.
    pub kind: EdgeEventKind,
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
            kind: EdgeEventKind::from(le.kind),
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
            kind: EdgeEventKind::from(le.kind),
            offset: le.offset,
            seqno: le.seqno,
            line_seqno: le.line_seqno,
        }
    }
}

/// The cause of an [`EdgeEvent`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum EdgeEventKind {
    /// Indicates the line transitioned from inactive to active.
    RisingEdge = 1,
    /// Indicates the line transitioned from active to inactive.
    FallingEdge = 2,
}
impl From<uv::LineEdgeEventKind> for EdgeEventKind {
    fn from(kind: uv::LineEdgeEventKind) -> Self {
        match kind {
            uv::LineEdgeEventKind::RisingEdge => EdgeEventKind::RisingEdge,
            uv::LineEdgeEventKind::FallingEdge => EdgeEventKind::FallingEdge,
        }
    }
}

/// The details of a change to the [`Info`] for a line.
#[derive(Debug)]
pub struct InfoChangeEvent {
    /// The updated line info.
    pub info: Info,
    /// The best estimate of time of event occurrence.
    pub timestamp: Timestamp,
    /// The trigger for the change.
    pub kind: InfoChangeKind,
}
#[cfg(feature = "uapi_v1")]
impl From<&v1::LineInfoChangeEvent> for InfoChangeEvent {
    fn from(ice: &v1::LineInfoChangeEvent) -> Self {
        InfoChangeEvent {
            info: Info::from(&ice.info),
            timestamp: Timestamp::from_nanos(ice.timestamp_ns),
            kind: InfoChangeKind::from(ice.kind),
        }
    }
}
#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
impl From<&v2::LineInfoChangeEvent> for InfoChangeEvent {
    fn from(ice: &v2::LineInfoChangeEvent) -> Self {
        InfoChangeEvent {
            info: Info::from(&ice.info),
            timestamp: Timestamp::from_nanos(ice.timestamp_ns),
            kind: InfoChangeKind::from(ice.kind),
        }
    }
}

/// The cause of a [`InfoChangeEvent`]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum InfoChangeKind {
    /// Line has been requested.
    Requested = 1,
    /// Line has been released.
    Released = 2,
    /// Line has been reconfigured.
    Reconfigured = 3,
}
impl From<uv::InfoChangeKind> for InfoChangeKind {
    fn from(kind: uv::InfoChangeKind) -> Self {
        match kind {
            uv::InfoChangeKind::Requested => InfoChangeKind::Requested,
            uv::InfoChangeKind::Released => InfoChangeKind::Released,
            uv::InfoChangeKind::Reconfigured => InfoChangeKind::Reconfigured,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(feature = "uapi_v1")]
    fn test_values_from_v1() {
        let offsets = Vec::from([1, 5, 3, 8]);
        let src = v1::LineValues::from_slice(&[1, 1, 0, 1]);
        let mut dst = Values::default();
        dst.set(4, Value::Active);
        dst.set(7, Value::Inactive);
        dst.from_v1(&offsets, &src);
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
    fn test_values_from_v2() {
        let offsets = Vec::from([1, 5, 3, 8]);
        let mut src = v2::LineValues::default();
        src.set(0, true);
        src.set(1, true);
        src.set(2, false);
        src.set(3, true);
        let mut dst = Values::default();
        dst.set(4, Value::Active);
        dst.set(7, Value::Inactive);
        dst.from_v2(&offsets, &src);
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
    fn test_values_to_v1() {
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
    #[cfg(feature = "uapi_v2")]
    fn test_values_to_v2() {
        let offsets = Vec::from([1, 5, 3, 8]);
        let mut src = Values::default();
        src.set(1, Value::Active);
        src.set(3, Value::Inactive);
        src.set(7, Value::Active); // should be ignored
        src.set(8, Value::Active);
        let dst = src.to_v2(&offsets);
        println!("dst {:?}", dst);
        assert!(dst.get(0).unwrap()); // 1
        assert!(dst.get(1).is_none()); // 5
        assert!(!dst.get(2).unwrap()); // 3
        assert!(dst.get(3).unwrap()); // 8
        assert_eq!(dst.mask.into_value(), 0b1101); // only 3 entries set
    }
}
