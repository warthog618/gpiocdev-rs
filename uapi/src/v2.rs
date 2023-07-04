// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use bitflags::bitflags;
use bitmaps::Bitmap;
use std::convert::TryFrom;
use std::fmt;
use std::fs::File;
use std::mem;
use std::os::unix::prelude::{FromRawFd, RawFd};
use std::time::Duration;

use super::common::{ValidationResult, IOCTL_MAGIC};

// common to ABI v1 and v2.
pub use super::common::{
    get_chip_info, unwatch_line_info, ChipInfo, LineEdgeEventKind, LineInfoChangeKind, Offset,
    Offsets, Padding, UnderReadError, ValidationError,
};
use super::{Error, Name, Result};

#[repr(u8)]
enum Ioctl {
    GetLineInfo = 5,
    WatchLineInfo = 6,
    GetLine = 7,
    SetLineConfig = 0xD,
    GetLineValues = 0xE,
    SetLineValues = 0xF,
}

bitflags! {
    /// Flags indicating the configuration of a line.
    #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
    pub struct LineFlags: u64 {
        /// The line is in use and is not available for request.
        const USED = 1;

        /// The line active state corresponds to a physical low.
        const ACTIVE_LOW = 2;

        /// The line is an input.
        const INPUT = 4;

        /// The line is an output.
        const OUTPUT = 8;

        /// The line detects rising (*inactive* to *active*) edges.
        const EDGE_RISING = 16;

        /// The line detects falling (*active* to *inactive*) edges.
        const EDGE_FALLING = 32;

        /// The line is an open drain output.
        const OPEN_DRAIN = 64;

        /// The line is an open source output.
        const OPEN_SOURCE = 128;

        /// The line has pull-up bias enabled.
        const BIAS_PULL_UP = 256;

        /// The line has pull-down bias enabled.
        const BIAS_PULL_DOWN = 512;

        /// The line has bias disabled.
        const BIAS_DISABLED = 1024;

        /// The line events contain **CLOCK_REALTIME** timestamps.
        const EVENT_CLOCK_REALTIME = 2048;

        /// The line events contain **HTE** timestamps.
        const EVENT_CLOCK_HTE = 4096;
    }
}

/// Values of GPIO lines.
///
/// Bits in the bitmaps correspond to the index into [`LineRequest.offsets`].
/// The first requested line, `offsets[0]`, is bit 0.
///
/// [`LineRequest.offsets`]: struct@LineRequest
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LineValues {
    /// The value of the lines, set to 1 for *active* and 0 for *inactive*.
    pub bits: Bitmap<64>,

    /// The lines in a request to access, set to 1 to access and 0 to ignore.
    pub mask: Bitmap<64>,
}

impl LineValues {
    /// Create values from a slice.
    ///
    /// The values are in the same order as [`LineRequest.offsets`].
    ///
    /// [`LineRequest.offsets`]: struct@LineRequest
    pub fn from_slice(s: &[bool]) -> Self {
        let mut lv: LineValues = Default::default();
        for (idx, val) in s.iter().enumerate() {
            lv.set(idx, *val);
        }
        lv
    }
    /// Return the value of a line.
    ///
    /// Note that the [`LineValues`] need to be populated via a call to [`get_line_values`]
    /// to get values from the underlying hardware.
    ///
    /// Fails if the line of interest is not set in the mask.
    ///
    /// * `idx` - The index into the [`LineRequest.offsets`] for the line of interest.
    ///
    /// [`LineRequest.offsets`]: struct@LineRequest
    pub fn get(&self, idx: usize) -> Option<bool> {
        if !self.mask.get(idx) {
            return None;
        }
        Some(self.bits.get(idx))
    }

    /// Set the value of a line.
    ///
    /// Note that the values are not applied to hardware until passed to [`set_line_values`].
    ///
    /// * `idx` - The index into the [`LineRequest.offsets`] for the line of interest.
    /// * `active` - The logical state of the line to be set.
    ///
    /// [`LineRequest.offsets`]: struct@LineRequest
    pub fn set(&mut self, idx: usize, active: bool) {
        self.bits.set(idx, active);
        self.mask.set(idx, true);
    }

    /// Clear the mask bit for a line.
    ///
    /// The line will be ignored in subsequent calls to [`get_line_values`] and
    /// [`set_line_values`].
    ///
    /// * `idx` - The index into the [`LineRequest.offsets`] for the line of interest.
    ///
    /// [`LineRequest.offsets`]: struct@LineRequest
    pub fn unset_mask(&mut self, idx: usize) {
        self.mask.set(idx, false);
    }
}

/// Read values of requested lines.
///
/// * `lfd` - The fd of the file returned by [`get_line`].
/// * `lv` - The line values to be populated.
pub fn get_line_values(lfd: RawFd, lv: &mut LineValues) -> Result<()> {
    // SAFETY: returned struct contains raw byte arrays and bitfields that are safe to decode.
    match unsafe {
        libc::ioctl(
            lfd,
            nix::request_code_readwrite!(
                IOCTL_MAGIC,
                Ioctl::GetLineValues,
                mem::size_of::<LineValues>()
            ),
            lv,
        )
    } {
        0 => Ok(()),
        _ => Err(Error::from_errno()),
    }
}

/// Set values of requested output lines.
///
/// Note that requesting a set on an input line is an error.
///
/// * `lfd` - The fd of the file returned by [`get_line`].
/// * `lv` - The line values to be set.
pub fn set_line_values(lfd: RawFd, lv: &LineValues) -> Result<()> {
    // SAFETY: lv is not modified.
    match unsafe {
        libc::ioctl(
            lfd,
            nix::request_code_readwrite!(
                IOCTL_MAGIC,
                Ioctl::SetLineValues,
                mem::size_of::<LineValues>()
            ),
            lv,
        )
    } {
        0 => Ok(()),
        _ => Err(Error::from_errno()),
    }
}

/// An identifier for which field of the [`LineAttributeValueUnion`] is in use.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LineAttributeKind {
    /// The attribute is *inactive* - no fields are in use.
    #[default]
    Unused = 0,

    /// The flags field is in use.
    Flags = 1,

    /// The values field is in use.
    Values = 2,

    /// The debounce_period_us field is in use.
    Debounce = 3,
}

impl TryFrom<u32> for LineAttributeKind {
    type Error = String;

    fn try_from(v: u32) -> std::result::Result<Self, Self::Error> {
        use LineAttributeKind::*;
        Ok(match v {
            x if x == Unused as u32 => Unused,
            x if x == Flags as u32 => Flags,
            x if x == Values as u32 => Values,
            x if x == Debounce as u32 => Debounce,
            x => return Err(format!("invalid value: {}", x)),
        })
    }
}

impl LineAttributeKind {
    /// Confirm that the value read from the kernel is valid in Rust.
    fn validate(&self) -> std::result::Result<(), String> {
        LineAttributeKind::try_from(*self as u32).map(|_i| ())
    }
}

/// A configurable attribute of a line.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LineAttribute {
    /// The type of attribute stored in `value`.
    pub kind: LineAttributeKind,

    /// Reserved for future use and must be zero filled.
    #[doc(hidden)]
    pub padding: Padding<1>,

    /// The attribute value.
    pub value: LineAttributeValueUnion,
}

impl LineAttribute {
    /// Set the attribute as debounce period.
    pub fn set_debounce_period_us(&mut self, debounce_period_us: u32) {
        self.kind = LineAttributeKind::Debounce;
        self.value.debounce_period_us = debounce_period_us;
    }

    /// Set the attribute as flags.
    pub fn set_flags(&mut self, flags: LineFlags) {
        self.kind = LineAttributeKind::Flags;
        self.value.flags = flags;
    }

    /// Set the attribute as output values.
    pub fn set_values(&mut self, values: Bitmap<64>) {
        self.kind = LineAttributeKind::Values;
        self.value.values = values;
    }

    /// Get the contained value.
    ///
    /// Converts the unsafe kind/union into a safe enum.
    pub fn to_value(&self) -> Option<LineAttributeValue> {
        // SAFETY: checks kind before accessing union
        unsafe {
            Some(match self.kind {
                LineAttributeKind::Unused => return None,
                LineAttributeKind::Flags => LineAttributeValue::Flags(self.value.flags),
                LineAttributeKind::Values => LineAttributeValue::Values(self.value.values),
                LineAttributeKind::Debounce => LineAttributeValue::DebouncePeriod(
                    Duration::from_micros(self.value.debounce_period_us as u64),
                ),
            })
        }
    }
}

impl fmt::Debug for LineAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: checks kind before accessing union
        unsafe {
            match self.kind {
                LineAttributeKind::Unused => write!(f, "unused"),
                LineAttributeKind::Flags => write!(f, "flags: {:?}", self.value.flags),
                LineAttributeKind::Values => {
                    write!(f, "values: {:08x}", self.value.values.as_value())
                }
                LineAttributeKind::Debounce => {
                    write!(f, "debounce_period_us: {}", self.value.debounce_period_us)
                }
            }
        }
    }
}

impl PartialEq for LineAttribute {
    fn eq(&self, other: &Self) -> bool {
        if self.kind != other.kind {
            return false;
        }
        // SAFETY: checks kind before accessing union
        unsafe {
            match self.kind {
                LineAttributeKind::Unused => true,
                LineAttributeKind::Flags => self.value.flags == other.value.flags,
                LineAttributeKind::Values => self.value.values == other.value.values,
                LineAttributeKind::Debounce => {
                    self.value.debounce_period_us == other.value.debounce_period_us
                }
            }
        }
    }
}
impl Eq for LineAttribute {}

/// The value of a particular line attribute.
#[repr(C)]
#[derive(Clone, Copy)]
pub union LineAttributeValueUnion {
    /// The line configuration flags.
    pub flags: LineFlags,

    /// The values to which the lines will be set, with each bit number
    ///  corresponding to the index into [`LineRequest.offsets`].
    ///
    /// [`LineRequest.offsets`]: struct@LineRequest
    pub values: Bitmap<64>,

    /// The debounce period, in microseconds.
    pub debounce_period_us: u32,
}

impl Default for LineAttributeValueUnion {
    fn default() -> Self {
        LineAttributeValueUnion {
            flags: Default::default(),
        }
    }
}

/// The attribute value contained within a [`LineAttribute`].
pub enum LineAttributeValue {
    /// The debounce period attribute as a Duration.
    DebouncePeriod(Duration),

    /// The configuration flags.
    Flags(LineFlags),

    /// The line values.
    Values(Bitmap<64>),
}

/// A configuration attribute associated with one or more of the requested lines.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LineConfigAttribute {
    /// The configurable attribute.
    pub attr: LineAttribute,

    /// The lines to which the attribute applies, with each bit number corresponding
    /// to the index into [`LineRequest.offsets`].
    ///
    /// [`LineRequest.offsets`]: struct@LineRequest
    pub mask: Bitmap<64>,
}

/// The set of additional configuration attributes for a line request.
///
/// [`LineConfig.num_attrs`] specifies the number of entries in use.
///
/// Any attribute should only be associated with a particular line once.
/// If an attribute is associated with a line multiple times then the
/// first occurrence (i.e. lowest index) has precedence.
///
/// [`LineConfig.num_attrs`]: struct@LineConfig
#[repr(C)]
#[derive(Clone, Debug, Default)]
pub struct LineConfigAttributes(pub [LineConfigAttribute; NUM_ATTRS_MAX]);

/// Configuration for a set of requested lines.
#[repr(C)]
#[derive(Clone, Debug, Default)]
pub struct LineConfig {
    /// Flags for the GPIO lines.  This is the default for all requested lines but
    /// may be overridden for particular lines using `attrs`.
    pub flags: LineFlags,

    /// The number of attributes active in `attrs`.
    pub num_attrs: u32,

    /// Reserved for future use and must be zero filled.
    #[doc(hidden)]
    pub padding: Padding<5>,

    /// The configuration attributes associated with the requested lines.
    ///
    /// The number of active attributes in the array is specified by `num_attrs`.
    pub attrs: LineConfigAttributes,
}

impl LineConfig {
    /// The nth attribute in the attrs
    pub fn attr(&self, idx: usize) -> &LineConfigAttribute {
        &self.attrs.0[idx]
    }

    /// The nth attribute in the attrs
    pub fn attr_mut(&mut self, idx: usize) -> &mut LineConfigAttribute {
        &mut self.attrs.0[idx]
    }
}

/// Update the configuration of an existing line request.
///
/// * `lfd` - The fd of the file returned by [`get_line`].
/// * `lc` - The configuration to be applied.
pub fn set_line_config(lfd: RawFd, lc: LineConfig) -> Result<()> {
    // SAFETY: lc is consumed.
    unsafe {
        match libc::ioctl(
            lfd,
            nix::request_code_readwrite!(
                IOCTL_MAGIC,
                Ioctl::SetLineConfig,
                mem::size_of::<LineConfig>()
            ),
            &lc,
        ) {
            0 => Ok(()),
            _ => Err(Error::from_errno()),
        }
    }
}

/// Information about a request for GPIO lines.
#[repr(C)]
#[derive(Clone, Debug, Default)]
pub struct LineRequest {
    /// An array of requested lines, identified by offset on the associated GPIO chip.
    pub offsets: Offsets,

    /// The requested consumer label for the selected GPIO lines such as
    /// "*my-bitbanged-relay*".
    pub consumer: Name,

    /// The requested configuration for the lines.
    pub config: LineConfig,

    /// The number of lines requested in this request.
    /// i.e. the number of valid elements in `offsets`.
    ///
    /// Set to 1 to request a single line.
    pub num_lines: u32,

    /// A suggested minimum number of line events that the kernel should buffer.
    ///
    /// This is only relevant if edge detection is enabled in the configuration.
    ///
    /// Note that this is only a suggested value and the kernel may allocate a
    /// larger buffer or cap the size of the buffer.
    /// If this field is zero then the buffer size defaults to a minimum of `num_lines*16`.
    pub event_buffer_size: u32,

    /// Reserved for future use and must be zero filled.
    #[doc(hidden)]
    pub padding: Padding<5>,

    /// This field is only present for the underlying ioctl call and is only used internally.
    #[doc(hidden)]
    pub fd: i32,
}

/// Request a line or set of lines for exclusive access.
///
/// * `cfd` - The fd of the open chip.
/// * `lr` - The line request.
pub fn get_line(cfd: RawFd, lr: LineRequest) -> Result<File> {
    // SAFETY: lr is consumed and the returned file is drawn from the returned fd.
    unsafe {
        match libc::ioctl(
            cfd,
            nix::request_code_readwrite!(
                IOCTL_MAGIC,
                Ioctl::GetLine,
                mem::size_of::<LineRequest>()
            ),
            &lr,
        ) {
            0 => Ok(File::from_raw_fd(lr.fd)),
            _ => Err(Error::from_errno()),
        }
    }
}

/// The set of potential configuration attributes for a line.
///
/// [`LineInfo.num_attrs`] specifies the number of entries in use.
///
/// [`LineInfo.num_attrs`]: struct@LineInfo
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LineAttributes([LineAttribute; NUM_ATTRS_MAX]);

/// The capacity of [`LineAttributes`] and [`LineConfigAttributes`] arrays.
pub const NUM_ATTRS_MAX: usize = 10;

/// Information about a certain GPIO line.
#[repr(C)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LineInfo {
    /// The name of this GPIO line, such as the output pin of the line on the chip,
    /// a rail or a pin header name on a board, as specified by the GPIO chip.
    ///
    /// May be empty.
    pub name: Name,

    /// A functional name for the consumer of this GPIO line as set by whatever is using it.
    ///
    /// Will be empty if there is no current user.
    /// May also be empty if the consumer requests doesn't set this up.
    pub consumer: Name,

    /// The local offset on this GPIO chip.
    pub offset: Offset,

    /// The number of attributes active in `attrs`.
    pub num_attrs: u32,

    /// The configuration flags for this GPIO line.
    pub flags: LineFlags,

    /// Additilnal configuration attributes associated with the line.
    ///
    /// The number of active attributes in the array is specified by `num_attrs`.
    pub attrs: LineAttributes,

    /// Reserved for future use.
    #[doc(hidden)]
    pub padding: Padding<4>,
}

impl LineInfo {
    /// The nth attribute in the attrs
    pub fn attr(&self, idx: usize) -> &LineAttribute {
        &self.attrs.0[idx]
    }
    /// Check that a LineInfo read from the kernel is valid in Rust.
    fn validate(&self) -> ValidationResult {
        if self.num_attrs > NUM_ATTRS_MAX as u32 {
            return Err(ValidationError::new(
                "num_attrs",
                format!("out of range: {}", self.num_attrs),
            ));
        }
        for i in 0..NUM_ATTRS_MAX {
            if let Err(e) = self.attrs.0[i].kind.validate() {
                return Err(ValidationError::new(format!("attrs[{}].kind", i), e));
            }
        }
        Ok(())
    }
}

/// Get the publicly available information for a line.
///
/// This does not include the line value.
/// The line must be requested to access the value.
///
/// * `cfd` - The fd of the open chip.
/// * `offset` - The offset of the line.
pub fn get_line_info(cfd: RawFd, offset: Offset) -> Result<LineInfo> {
    let li = LineInfo {
        offset,
        ..Default::default()
    };
    // SAFETY: returned struct is explicitly validated before being returned.
    match unsafe {
        libc::ioctl(
            cfd,
            nix::request_code_readwrite!(
                IOCTL_MAGIC,
                Ioctl::GetLineInfo,
                mem::size_of::<LineInfo>()
            ),
            &li,
        )
    } {
        0 => li.validate().map(|_| li).map_err(Error::from),
        _ => Err(Error::from_errno()),
    }
}

/// Add a watch on changes to the [`LineInfo`] for a line.
///
/// Returns the current state of that information.
///
/// This does not include the line value.
/// The line must be requested to access the value.
///
/// * `cfd` - The fd of the open chip.
/// * `offset` - The offset of the line to watch.
pub fn watch_line_info(cfd: RawFd, offset: Offset) -> Result<LineInfo> {
    let li = LineInfo {
        offset,
        ..Default::default()
    };
    // SAFETY: returned struct is explicitly validated before being returned.
    match unsafe {
        libc::ioctl(
            cfd,
            nix::request_code_readwrite!(
                IOCTL_MAGIC,
                Ioctl::WatchLineInfo,
                mem::size_of::<LineInfo>()
            ),
            &li,
        )
    } {
        0 => li.validate().map(|_| li).map_err(Error::from),
        _ => Err(Error::from_errno()),
    }
}

/// An event indicating a change to the info for a line.
#[repr(C)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineInfoChangeEvent {
    /// The new line info.
    pub info: LineInfo,

    /// The best estimate of time of event occurrence, in nanoseconds.
    pub timestamp_ns: u64,

    /// The trigger for the change.
    pub kind: LineInfoChangeKind,

    /// Reserved for future use.
    #[doc(hidden)]
    pub padding: Padding<5>,
}

impl LineInfoChangeEvent {
    /// Read an info change event from a buffer.
    ///
    /// The buffer is assumed to have been populated by a read of the chip File,
    /// so the content is validated before being returned.
    pub fn from_slice(d: &[u64]) -> Result<&LineInfoChangeEvent> {
        debug_assert!(mem::size_of::<LineInfoChangeEvent>() % 8 == 0);
        let len = d.len() * 8;
        if len < mem::size_of::<LineInfoChangeEvent>() {
            return Err(Error::from(UnderReadError::new(
                "LineInfoChangeEvent",
                mem::size_of::<LineInfoChangeEvent>(),
                len,
            )));
        }
        // SAFETY: returned struct is explicitly validated before being returned.
        let ice = unsafe { &*(d as *const [u64] as *const LineInfoChangeEvent) };
        ice.validate().map(|_| ice).map_err(Error::from)
    }

    /// Check that a LineInfoChangeEvent read from the kernel is valid in Rust.
    fn validate(&self) -> ValidationResult {
        self.kind
            .validate()
            .map_err(|e| ValidationError::new("kind", e))
    }
}

/// Information about an edge event on a requested line.
#[repr(C)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineEdgeEvent {
    /// The best estimate of time of event occurrence, in nanoseconds.
    ///
    /// By default the timestamp is read from **CLOCK_MONOTONIC** and is
    /// intended to allow the accurate measurement of the time between events.
    /// It does not provide the wall-clock time.
    ///
    /// If the [`LineFlags::EVENT_CLOCK_REALTIME`] flag is set then the
    /// timestamp is read from **CLOCK_REALTIME**.
    pub timestamp_ns: u64,

    /// The event trigger identifier.
    pub kind: LineEdgeEventKind,

    /// The offset of the line that triggered the event.
    pub offset: Offset,

    /// The sequence number for this event in the sequence of events for all
    /// the lines in this line request.
    pub seqno: u32,

    /// The sequence number for this event in the sequence of events on this
    /// particular line.
    pub line_seqno: u32,

    /// Reserved for future use.
    #[doc(hidden)]
    pub padding: Padding<6>,
}

impl LineEdgeEvent {
    /// Read an edge event from a buffer.
    ///
    /// The buffer is assumed to have been populated by a read of the line request File,
    /// so the content is validated before being returned.
    pub fn from_slice(d: &[u64]) -> Result<&LineEdgeEvent> {
        debug_assert!(mem::size_of::<LineEdgeEvent>() % 8 == 0);
        let len = d.len() * 8;
        if len < mem::size_of::<LineEdgeEvent>() {
            return Err(Error::from(UnderReadError::new(
                "LineEdgeEvent",
                mem::size_of::<LineEdgeEvent>(),
                len,
            )));
        }
        // SAFETY: returned struct is explicitly validated before being returned.
        let le = unsafe { &*(d as *const [u64] as *const LineEdgeEvent) };
        le.validate().map(|_| le).map_err(Error::from)
    }

    /// Check that a LineEdgeEvent read from the kernel is valid in Rust.
    fn validate(&self) -> ValidationResult {
        self.kind
            .validate()
            .map_err(|e| ValidationError::new("kind", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod line_attribute {
        use super::LineAttribute;

        #[test]
        fn size() {
            assert_eq!(
                super::mem::size_of::<LineAttribute>(),
                16usize,
                concat!("Size of: ", stringify!(LineAttribute))
            );
        }
    }

    mod line_attribute_value_union {
        use super::LineAttributeValueUnion;

        #[test]
        fn size() {
            assert_eq!(
                super::mem::size_of::<LineAttributeValueUnion>(),
                8usize,
                concat!("Size of: ", stringify!(LineAttributeValueUnion))
            );
        }
    }

    mod line_config_attribute {
        use super::LineConfigAttribute;

        #[test]
        fn line_config_attribute() {
            assert_eq!(
                super::mem::size_of::<LineConfigAttribute>(),
                24usize,
                concat!("Size of: ", stringify!(LineConfigAttribute))
            );
        }
    }

    mod line_config {
        use super::LineConfig;

        #[test]
        fn line_config() {
            assert_eq!(
                super::mem::size_of::<LineConfig>(),
                272usize,
                concat!("Size of: ", stringify!(LineConfig))
            );
        }
    }

    mod line_request {
        use super::LineRequest;

        #[test]
        fn line_request() {
            assert_eq!(
                super::mem::size_of::<LineRequest>(),
                592usize,
                concat!("Size of: ", stringify!(LineRequest))
            );
        }
    }

    mod line_values {
        use super::{Bitmap, LineValues};

        #[test]
        fn get() {
            let mut a = LineValues::default();
            for idx in [0, 2] {
                assert!(!a.bits.get(idx), "idx: {}", idx);
                assert!(a.get(idx).is_none(), "idx: {}", idx);

                a.mask.set(idx, true);
                assert!(!a.get(idx).unwrap(), "idx: {}", idx);

                a.bits.set(idx, true);
                assert!(a.get(idx).unwrap(), "idx: {}", idx);
            }
        }

        #[test]
        fn set() {
            let mut a = LineValues::default();
            for idx in [0, 2] {
                a.set(idx, false);
                assert!(a.mask.get(idx), "idx: {}", idx);
                assert!(!a.bits.get(idx), "idx: {}", idx);

                a.set(idx, true);
                assert!(a.mask.get(idx), "idx: {}", idx);
                assert!(a.bits.get(idx), "idx: {}", idx);
            }
        }

        #[test]
        fn unset_mask() {
            let mut a = LineValues {
                mask: Bitmap::mask(6),
                ..Default::default()
            };
            assert_eq!(a.mask.len(), 6);
            assert!(a.mask.get(0));

            a.unset_mask(0);
            assert!(!a.mask.get(0));
            assert_eq!(a.mask.len(), 5);
            assert!(a.mask.get(3));

            a.unset_mask(3);
            assert!(!a.mask.get(3));
            assert_eq!(a.mask.len(), 4);
            assert!(a.mask.get(5));

            a.unset_mask(5);
            assert!(!a.mask.get(5));
            assert_eq!(a.mask.len(), 3);
            assert!(!a.mask.get(6));

            a.unset_mask(5);
            assert!(!a.mask.get(5));
            assert_eq!(a.mask.len(), 3);
        }

        #[test]
        fn size() {
            assert_eq!(
                super::mem::size_of::<LineValues>(),
                16usize,
                concat!("Size of: ", stringify!(LineValues))
            );
        }
    }

    mod line_attribute_kind {
        use super::LineAttributeKind;

        #[test]
        fn validate() {
            let mut a = LineAttributeKind::Flags;
            assert!(a.validate().is_ok());

            a = LineAttributeKind::Unused;
            assert!(a.validate().is_ok());

            unsafe {
                a = *(&4 as *const i32 as *const LineAttributeKind);
                assert_eq!(a.validate().unwrap_err(), "invalid value: 4");
                a = *(&3 as *const i32 as *const LineAttributeKind);
                assert!(a.validate().is_ok());
            }
        }
    }

    mod line_info {
        use super::{LineAttributeKind, LineInfo, NUM_ATTRS_MAX};

        #[test]
        fn validate() {
            let mut a = LineInfo::default();
            assert!(a.validate().is_ok());

            a.num_attrs = NUM_ATTRS_MAX as u32;
            assert!(a.validate().is_ok());

            a.num_attrs += 1;
            let e = a.validate().unwrap_err();
            assert_eq!(e.field, "num_attrs");
            assert_eq!(e.msg, "out of range: 11");

            a.num_attrs = NUM_ATTRS_MAX as u32;
            for idx in [0, 1, 4, 7] {
                unsafe {
                    a.attrs.0[idx].kind = *(&4 as *const i32 as *const LineAttributeKind);
                }
                let e = a.validate().unwrap_err();
                assert_eq!(e.field, format!("attrs[{}].kind", idx));
                assert_eq!(e.msg, "invalid value: 4");
                a.attrs.0[idx].kind = LineAttributeKind::Unused;
            }
        }

        #[test]
        fn size() {
            assert_eq!(
                super::mem::size_of::<LineInfo>(),
                256usize,
                concat!("Size of: ", stringify!(LineInfo))
            );
        }
    }

    mod line_info_changed {
        use super::{LineInfoChangeEvent, LineInfoChangeKind};

        #[test]
        fn validate() {
            let mut a = LineInfoChangeEvent {
                info: Default::default(),
                timestamp_ns: 0,
                kind: LineInfoChangeKind::Released,
                padding: Default::default(),
            };
            assert!(a.validate().is_ok());

            a.timestamp_ns = 1234;
            assert!(a.validate().is_ok());
            unsafe {
                a.kind = *(&0 as *const i32 as *const LineInfoChangeKind);
                let e = a.validate().unwrap_err();
                assert_eq!(e.field, "kind");
                assert_eq!(e.msg, "invalid value: 0");

                a.kind = *(&4 as *const i32 as *const LineInfoChangeKind);
                let e = a.validate().unwrap_err();
                assert_eq!(e.field, "kind");
                assert_eq!(e.msg, "invalid value: 4");

                a.kind = *(&1 as *const i32 as *const LineInfoChangeKind);
                assert!(a.validate().is_ok());
            }
        }
        #[test]
        fn size() {
            assert_eq!(
                super::mem::size_of::<LineInfoChangeEvent>(),
                288usize,
                concat!("Size of: ", stringify!(LineInfoChangeEvent))
            );
        }
    }

    mod line_event {
        use super::{LineEdgeEvent, LineEdgeEventKind};

        #[test]
        fn validate() {
            let mut a = LineEdgeEvent {
                timestamp_ns: 0,
                kind: LineEdgeEventKind::RisingEdge,
                offset: 0,
                seqno: 0,
                line_seqno: 0,
                padding: Default::default(),
            };
            assert!(a.validate().is_ok());

            a.timestamp_ns = 1234;
            assert!(a.validate().is_ok());
            unsafe {
                a.kind = *(&0 as *const i32 as *const LineEdgeEventKind);
                let e = a.validate().unwrap_err();
                assert_eq!(e.field, "kind");
                assert_eq!(e.msg, "invalid value: 0");

                a.kind = *(&3 as *const i32 as *const LineEdgeEventKind);
                let e = a.validate().unwrap_err();
                assert_eq!(e.field, "kind");
                assert_eq!(e.msg, "invalid value: 3");

                a.kind = *(&1 as *const i32 as *const LineEdgeEventKind);
                assert!(a.validate().is_ok());
            }
        }

        #[test]
        fn size() {
            assert_eq!(
                super::mem::size_of::<LineEdgeEvent>(),
                48usize,
                concat!("Size of: ", stringify!(LineEdgeEvent))
            );
        }
    }
}
