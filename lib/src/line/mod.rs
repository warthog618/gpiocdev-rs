// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod config;
pub use self::config::Config;

mod event;
pub use self::event::{EdgeEvent, EdgeKind, InfoChangeEvent, InfoChangeKind};

mod info;
pub use self::info::Info;

mod value;
pub use self::value::{Value, Values};

#[cfg(feature = "uapi_v1")]
use gpiocdev_uapi::v1;
#[cfg(feature = "uapi_v2")]
use gpiocdev_uapi::v2;
#[cfg(feature = "serde")]
use serde_derive::{Deserialize, Serialize};

/// An identifier for a line on a particular chip.
///
/// Valid offsets are in the range 0..`num_lines` as reported in the chip [`Info`](super::chip::Info).
pub type Offset = u32;

/// A collection of line offsets.
pub type Offsets = Vec<Offset>;

/// The direction of a line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum EventClock {
    /// The **CLOCK_MONOTONIC** is used as the source for edge event timestamps.
    ///
    /// This is the default for ABI v2.
    Monotonic,

    /// The **CLOCK_REALTIME** is used as the source for edge event timestamps.
    Realtime,

    /// The hardware timestamp engine provides event timestamps.
    ///
    /// This source requires a Linux kernel 5.19 or later with CONFIG_HTE
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
