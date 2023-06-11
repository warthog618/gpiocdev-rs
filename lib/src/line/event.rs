// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{Info, Offset};
#[cfg(all(feature = "uapi_v1", not(feature = "uapi_v2")))]
use gpiocdev_uapi::v1 as uapi;
#[cfg(feature = "uapi_v1")]
use gpiocdev_uapi::v1;
#[cfg(feature = "uapi_v2")]
use gpiocdev_uapi::{v2, v2 as uapi};
#[cfg(feature = "serde")]
use serde_derive::{Deserialize, Serialize};

/// The details of an edge detected on an input line.
///
/// ABI v1 does not provide the seqno nor line_seqno fields.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EdgeEvent {
    /// The best estimate of time of event occurrence, in nanoseconds.
    ///
    /// The interpretation of this field depends on line [`EventClock`](super::EventClock)
    /// configuration and uAPI ABI version, and so is left raw here.
    ///
    /// **CLOCK_REALTIME** can be converted to [`Timestamp`](crate::Timestamp).
    ///
    /// **CLOCK_MONOTONIC** is intended for comparing times between events and
    /// should be converted to [`Duration`](std::time::Duration).
    pub timestamp_ns: u64,

    /// The event trigger identifier.
    pub kind: EdgeKind,

    /// The offset of the line that triggered the event.
    pub offset: Offset,

    /// The sequence number for this event in the sequence of events for all
    /// the lines in this line request.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_zero"))]
    pub seqno: u32,

    /// The sequence number for this event in the sequence of events on this
    /// particular line.
    #[cfg_attr(
        feature = "serde",
        serde(rename = "lineSeqno", skip_serializing_if = "is_zero")
    )]
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

#[cfg(feature = "serde")]
fn is_zero(u: &u32) -> bool {
    *u == 0
}

/// The cause of an [`EdgeEvent`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InfoChangeEvent {
    /// The updated line info.
    #[cfg_attr(feature = "serde", serde(flatten))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    use crate::line::Drive;

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
                kind: gpiocdev_uapi::v2::LineInfoChangeKind::Reconfigured,
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
