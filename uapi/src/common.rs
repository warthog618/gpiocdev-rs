// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use core::ptr::null;
use libc::{c_long, ioctl, pollfd, ppoll, sigset_t, time_t, timespec, POLLIN};
use std::ffi::OsStr;
use std::fs::File;
use std::io::Error as IoError;
use std::mem::{size_of, MaybeUninit};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::AsRawFd;
use std::slice;
use std::str::FromStr;
use std::str::Utf8Error;
use std::time::Duration;

/// Check if the file has an event available to read.
pub fn has_event(f: &mut File) -> Result<bool> {
    wait_event(f, Duration::ZERO)
}

/// Wait for the file to have an event available to read.
pub fn wait_event(f: &mut File, d: Duration) -> Result<bool> {
    let mut pfd = pollfd {
        fd: f.as_raw_fd(),
        events: POLLIN,
        revents: 0,
    };
    let timeout = timespec {
        tv_sec: d.as_secs() as time_t,
        tv_nsec: d.subsec_nanos() as c_long,
    };
    unsafe {
        match ppoll(
            std::ptr::addr_of_mut!(pfd),
            1,
            std::ptr::addr_of!(timeout),
            null() as *const sigset_t,
        ) {
            -1 => Err(Error::from(IoError::last_os_error())),
            0 => Ok(false),
            _ => Ok(true),
        }
    }
}

pub(crate) const IOCTL_MAGIC: u8 = 0xb4;

#[repr(u8)]
enum Ioctl {
    GetChipInfo = 1,
    UnwatchLineInfo = 0xC,
}

/// Information about a particular GPIO chip.
#[repr(C)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChipInfo {
    /// The Linux kernel name of this GPIO chip.
    pub name: Name,
    /// A functional name for this GPIO chip, such as a product number.
    ///
    /// May be empty.
    pub label: Name,
    /// The number of GPIO lines on this chip.
    pub num_lines: u32,
}

/// Get the publicly available information for a chip.
///
/// * `cf` - The open chip File.
pub fn get_chip_info(cf: &File) -> Result<ChipInfo> {
    let mut chip = MaybeUninit::<ChipInfo>::uninit();
    unsafe {
        match ioctl(
            cf.as_raw_fd(),
            nix::request_code_read!(IOCTL_MAGIC, Ioctl::GetChipInfo, size_of::<ChipInfo>()),
            chip.as_mut_ptr(),
        ) {
            0 => Ok(chip.assume_init()),
            _ => Err(Error::from(IoError::last_os_error())),
        }
    }
}

/// Remove any watch on changes to the [`LineInfo`] for a line.
///
/// * `cf` - The open chip File.
/// * `offset` - The offset of the line to unwatch.
///
/// [`LineInfo`]: struct.LineInfo.html
pub fn unwatch_line_info(cf: &File, offset: Offset) -> Result<()> {
    match unsafe {
        ioctl(
            cf.as_raw_fd(),
            nix::request_code_readwrite!(IOCTL_MAGIC, Ioctl::UnwatchLineInfo, size_of::<u32>()),
            offset,
        )
    } {
        0 => Ok(()),
        _ => Err(Error::from(IoError::last_os_error())),
    }
}

/// The result returned by [`gpiod_uapi`] functions.
///
/// [`gpiod_uapi`]: crate
pub type Result<T> = std::result::Result<T, Error>;

/// Result returned by struct validators.
pub type ValidationResult = std::result::Result<(), ValidationError>;

/// Errors returned by [`gpiod_uapi`] functions.
///
/// [`gpiod_uapi`]: crate
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An error returned from an underlying system call.
    #[error(transparent)]
    Os(#[from] std::io::Error),
    #[error(transparent)]
    Validation(#[from] ValidationError),
}

/// A failure to validate a struct returned from a system call.
//
// Should only be seen if a kernel update adds an enum value we are unaware of.
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
#[error("Kernel returned invalid {field}: {msg}")]
pub struct ValidationError {
    pub field: String,
    pub msg: String,
}

impl ValidationError {
    pub fn new<S: Into<String>, T: Into<String>>(field: S, msg: T) -> ValidationError {
        ValidationError {
            field: field.into(),
            msg: msg.into(),
        }
    }
}

/// The maximum number of bytes stored in a Name.
pub const NAME_MAX: usize = 32;

/// A uAPI name string, common to ABI v1 and v2.
#[repr(C)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Name([u8; NAME_MAX]);

impl Name {
    /// Checks whether the Name is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0[0] == 0
    }
    /// The length of the contained name.
    #[inline]
    pub fn strlen(&self) -> usize {
        self.0.iter().position(|&x| x == 0).unwrap_or(self.0.len())
    }
    /// Convert the contained name to a OsString slice.
    pub fn as_os_str(&self) -> &OsStr {
        unsafe { OsStr::from_bytes(slice::from_raw_parts(&self.0[0], self.strlen())) }
    }
    /// Construct a Name from byte slice.
    ///
    /// May result in invalid UTF-8 if truncated in the middle of a multi-byte character.
    pub fn from_bytes(s: &[u8]) -> Name {
        let mut d: Name = Default::default();
        for (src, dst) in s.iter().zip(d.0.iter_mut()) {
            *dst = *src;
        }
        d
    }
}

impl FromStr for Name {
    type Err = Utf8Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let mut n: Name = Default::default();
        for (src, dst) in s.as_bytes().iter().zip(n.0.iter_mut()) {
            *dst = *src;
        }
        Ok(n)
    }
}

/// An identifier for a line on a particular chip.
///
/// Valid offsets are in the range 0..`num_lines` as reported in the [`ChipInfo`].
pub type Offset = u32;

/// The maximum number of lines that may be requested in a single request.
pub const LINES_MAX: usize = 64;

/// A collection of line offsets.
///
/// Typically used to identify the lines belonging to a particular request.
#[repr(C)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Offsets([Offset; LINES_MAX]);

impl Offsets {
    /// Create offsets from an iterable list.
    pub fn from_slice(s: &[u32]) -> Self {
        let mut n: Offsets = Default::default();
        for (src, dst) in s.iter().zip(n.0.iter_mut()) {
            *dst = *src;
        }
        n
    }

    /// Get the indexed offset from the set.
    #[inline]
    pub fn get(&self, idx: usize) -> Offset {
        self.0[idx]
    }

    /// Set the indexed offset in the set.
    #[inline]
    pub fn set(&mut self, idx: usize, offset: Offset) {
        self.0[idx] = offset;
    }
}

impl Default for Offsets {
    fn default() -> Self {
        Offsets([0; LINES_MAX])
    }
}

/// Space reserved for future use.
///
/// Sized in multiples of u32 words.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct Padding<const SIZE: usize>([u32; SIZE]);

impl<const SIZE: usize> Default for Padding<SIZE> {
    fn default() -> Self {
        Padding([0; SIZE])
    }
}

impl<const SIZE: usize> Padding<SIZE> {
    pub fn is_zeroed(&self) -> bool {
        for x in self.0.iter() {
            if *x != 0 {
                return false;
            }
        }
        true
    }
}

/// The trigger identifier for a [`LineInfoChangeEvent`].
///
/// [`LineInfoChangeEvent`]: struct.LineInfoChangeEvent.html

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InfoChangeKind {
    /// The line has been requested.
    Requested = 1,
    /// The line has been released.
    Released = 2,
    /// The line has been reconfigured.
    Reconfigured = 3,
}

impl TryFrom<u32> for InfoChangeKind {
    type Error = String;

    fn try_from(v: u32) -> std::result::Result<Self, Self::Error> {
        use InfoChangeKind::*;
        match v {
            x if x == Requested as u32 => Ok(Requested),
            x if x == Released as u32 => Ok(Released),
            x if x == Reconfigured as u32 => Ok(Reconfigured),
            x => Err(format!("invalid value: {}", x)),
        }
    }
}

impl InfoChangeKind {
    /// Confirm that the value read from the kernel is valid in Rust.
    pub(crate) fn validate(&self) -> std::result::Result<(), String> {
        InfoChangeKind::try_from(*self as u32).map(|_i| ())
    }
}

/// The trigger identifier for a [`LineEdgeEvent`].
///
/// [`LineEdgeEvent`]: struct.LineEdgeEvent.html
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineEdgeEventKind {
    /// Indicates the line transitioned from *inactive* to *active*.
    RisingEdge = 1,
    /// Indicates the line transitioned from *active* to *inactive*.
    FallingEdge = 2,
}

impl TryFrom<u32> for LineEdgeEventKind {
    type Error = String;

    fn try_from(v: u32) -> std::result::Result<Self, Self::Error> {
        use LineEdgeEventKind::*;
        match v {
            x if x == RisingEdge as u32 => Ok(RisingEdge),
            x if x == FallingEdge as u32 => Ok(FallingEdge),
            _ => Err(format!("invalid value: {}", v)),
        }
    }
}

impl LineEdgeEventKind {
    /// Confirm that the value read from the kernel is valid in Rust.
    pub(crate) fn validate(&self) -> std::result::Result<(), String> {
        LineEdgeEventKind::try_from(*self as u32).map(|_i| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_chip_info() {
        assert_eq!(
            size_of::<ChipInfo>(),
            68usize,
            concat!("Size of: ", stringify!(ChipInfo))
        );
    }
    #[test]
    fn test_line_info_changed_kind_validate() {
        let mut a = InfoChangeKind::Requested;
        assert!(a.validate().is_ok());
        unsafe {
            a = *(&0 as *const i32 as *const InfoChangeKind);
            assert_eq!(a.validate().unwrap_err(), "invalid value: 0");
            a = *(&4 as *const i32 as *const InfoChangeKind);
            assert_eq!(a.validate().unwrap_err(), "invalid value: 4");
            a = *(&3 as *const i32 as *const InfoChangeKind);
            assert!(a.validate().is_ok());
        }
    }
    #[test]
    fn test_line_event_kind_validate() {
        let mut a = InfoChangeKind::Requested;
        assert!(a.validate().is_ok());
        unsafe {
            a = *(&0 as *const i32 as *const InfoChangeKind);
            assert_eq!(a.validate().unwrap_err(), "invalid value: 0");
            a = *(&4 as *const i32 as *const InfoChangeKind);
            assert_eq!(a.validate().unwrap_err(), "invalid value: 4");
            a = *(&3 as *const i32 as *const InfoChangeKind);
            assert!(a.validate().is_ok());
        }
    }
    #[test]
    fn test_name_from_str() {
        let mut x = [0u8; 32];
        x[0] = 98;
        x[1] = 97;
        x[2] = 110;
        x[3] = 97;
        x[4] = 110;
        x[5] = 97;
        let mut a = Name::from_str("banana").unwrap();
        assert_eq!(a.0, x);
        a = Name::from_str("apple").unwrap();
        x[0] = 97;
        x[1] = 112;
        x[2] = 112;
        x[3] = 108;
        x[4] = 101;
        x[5] = 0;
        assert_eq!(a.0, x);
    }
    #[test]
    fn test_name_is_empty() {
        let mut a = Name::default();
        assert!(a.is_empty());
        a = Name::from_str("banana").unwrap();
        assert!(!a.is_empty());
    }
    #[test]
    fn test_name_strlen() {
        let mut a = Name::default();
        assert_eq!(a.strlen(), 0);
        a = Name::from_str("banana").unwrap();
        assert_eq!(a.strlen(), 6);
        a = Name::from_str("an overly long truncated name -><- cut here").unwrap();
        assert_eq!(a.strlen(), 32);
    }
    #[test]
    fn test_name_as_os_str() {
        let mut a = Name::default();
        assert_eq!(a.as_os_str(), "");
        a = Name::from_str("banana").unwrap();
        assert_eq!(a.as_os_str(), "banana");
        a = Name::from_str("an overly long truncated name -><- cut here").unwrap();
        assert_eq!(a.as_os_str(), "an overly long truncated name ->");
    }
    #[test]
    fn test_name_default() {
        assert_eq!(Name::default().0, [0u8; NAME_MAX]);
    }
    #[test]
    fn test_offsets_from_slice() {
        let mut x = [0u32; LINES_MAX];
        x[0] = 1;
        x[1] = 2;
        x[2] = 3;
        x[3] = 0;
        x[4] = 5;
        x[5] = 6;
        let mut a = Offsets::from_slice(&[1, 2, 3, 0, 5, 6]);
        assert_eq!(a.0, x);
        a = Offsets::from_slice(&[0, 1, 2, 3]);
        x[0] = 0;
        x[1] = 1;
        x[2] = 2;
        x[3] = 3;
        x[4] = 0;
        x[5] = 0;
        assert_eq!(a.0, x);
        a = Offsets::from_slice(&[0, 3, 2, 3, 4]);
        x[0] = 0;
        x[1] = 3;
        x[2] = 2;
        x[3] = 3;
        x[4] = 4;
        x[5] = 0;
        assert_eq!(a.0, x);
    }
    #[test]
    fn test_offsets_default() {
        assert_eq!(Offsets::default().0, [0u32; LINES_MAX]);
    }
    #[test]
    fn test_padding_is_zeroed() {
        let mut padding: Padding<3> = Padding::default();
        assert!(padding.is_zeroed());
        padding.0[1] = 3;
        assert!(!padding.is_zeroed());
    }
    #[test]
    fn test_size_name() {
        assert_eq!(
            size_of::<Name>(),
            NAME_MAX,
            concat!("Size of: ", stringify!(Name))
        );
    }
    #[test]
    fn test_size_offsets() {
        assert_eq!(
            size_of::<Offsets>(),
            256usize,
            concat!("Size of: ", stringify!(Offsets))
        );
    }
    #[test]
    fn test_size_padding() {
        assert_eq!(
            size_of::<Padding<1>>(),
            4usize,
            concat!("Size of: ", stringify!(Padding<1>))
        );
        assert_eq!(
            size_of::<Padding<2>>(),
            8usize,
            concat!("Size of: ", stringify!(Padding<2>))
        );
        assert_eq!(
            size_of::<Padding<5>>(),
            20usize,
            concat!("Size of: ", stringify!(Padding<5>))
        );
    }
}
