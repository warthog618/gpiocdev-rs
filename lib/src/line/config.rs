// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{Bias, Direction, Drive, EdgeDetection, EventClock, Value};

#[cfg(feature = "uapi_v1")]
use gpiocdev_uapi::v1;
#[cfg(feature = "uapi_v2")]
use gpiocdev_uapi::v2;
#[cfg(feature = "serde")]
use serde_derive::{Deserialize, Serialize};
use std::time::Duration;

/// The configuration settings for a single line.
///
// Note it does not contain the offset to allow it to be applied to multiple lines.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

#[cfg(test)]
mod tests {
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
