// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{Bias, Direction, Drive, EdgeDetection, EventClock, Offset};
#[cfg(feature = "uapi_v2")]
use crate::{Error, UapiField};
#[cfg(feature = "uapi_v1")]
use gpiocdev_uapi::v1;
#[cfg(feature = "uapi_v2")]
use gpiocdev_uapi::v2;
#[cfg(feature = "serde")]
use serde_derive::{Deserialize, Serialize};
use std::time::Duration;

/// The publicly available information for a line.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct Info {
    /// The line offset on the GPIO chip.
    pub offset: Offset,

    /// The name of this GPIO line, such as the output pin of the line on
    /// the chip, a rail or a pin header name on a board, as specified by the
    /// GPIO chip.
    ///
    /// May be empty.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "String::is_empty"))]
    pub name: String,

    /// A functional name for the consumer of this GPIO line as set
    /// by whatever is using it.
    ///
    /// May be empty if not set by the user or the line is unused.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "String::is_empty"))]
    pub consumer: String,

    /// When true the line is used and not available for request.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub used: bool,

    /// When true the line active state corresponds to a physical low.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub active_low: bool,

    /// The direction of the line.
    pub direction: Direction,

    /// The bias state of the line.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub bias: Option<Bias>,

    /// The drive applied to output lines.
    ///
    /// Only relevant for output lines.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub drive: Option<Drive>,

    /// The edge detection state for the line.
    ///
    /// Only relevant for input lines.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub edge_detection: Option<EdgeDetection>,

    /// The source clock for edge event timestamps.
    ///
    /// Only relevant for input lines with edge detection.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub event_clock: Option<EventClock>,

    /// The debounce period.
    ///
    /// Only relevant for input lines with edge detection.
    ///
    /// None or a zero value means no debounce.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub debounce_period: Option<Duration>,
}

#[cfg(feature = "serde")]
fn is_false(b: &bool) -> bool {
    !b
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
impl TryFrom<&v2::LineInfo> for Info {
    type Error = Error;

    fn try_from(li: &v2::LineInfo) -> Result<Self, Self::Error> {
        let mut debounce_period = None;
        // range check num_attrs
        if li.num_attrs > v2::NUM_ATTRS_MAX as u32 {
            return Err(Error::UnexpectedResponse(
                UapiField::NumAttrs,
                format!("{}", li.num_attrs),
            ));
        }
        for idx in 0..li.num_attrs as usize {
            // change to a match if more attr types are added...
            if let Some(v2::LineAttributeValue::DebouncePeriod(db)) = li.attr(idx).to_value() {
                debounce_period = Some(Duration::from_micros(db as u64));
            }
        }
        let ed = EdgeDetection::try_from(li.flags).ok();
        let ec = if ed.is_some() {
            Some(EventClock::from(li.flags))
        } else {
            None
        };
        Ok(Info {
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
        })
    }
}

#[cfg(test)]
mod tests {
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
    fn info_try_from_v2_line_info() {
        let v2info: v2::LineInfo = Default::default();
        let info = Info::try_from(&v2info).expect("info should be valid");
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
        let info = Info::try_from(&v2info).expect("info should be valid");
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
        let info = Info::try_from(&v2info).expect("info should be valid");
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

        let mut v2info = v2::LineInfo {
            offset: 32,
            flags: v2::LineFlags::USED
                | v2::LineFlags::INPUT
                | v2::LineFlags::EDGE_RISING
                | v2::LineFlags::BIAS_PULL_DOWN,
            name: "banana".into(),
            consumer: "jam".into(),
            num_attrs: 1,
            attrs: Default::default(),
            padding: Default::default(),
        };
        v2info.attr_mut(0).set_debounce_period_us(24);

        let info = Info::try_from(&v2info).expect("info should be valid");
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
        assert_eq!(info.debounce_period, Some(Duration::from_micros(24)));

        let v2info = v2::LineInfo {
            offset: 32,
            flags: v2::LineFlags::USED
                | v2::LineFlags::INPUT
                | v2::LineFlags::EDGE_RISING
                | v2::LineFlags::BIAS_PULL_DOWN,
            name: "banana".into(),
            consumer: "jam".into(),
            num_attrs: 11,
            attrs: Default::default(),
            padding: Default::default(),
        };
        let err = Info::try_from(&v2info).expect_err("info should be invalid");
        assert_eq!(
            err,
            Error::UnexpectedResponse(UapiField::NumAttrs, "11".into())
        );
    }
}
