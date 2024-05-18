// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsStr;
use std::fs::File;
use std::os::unix::prelude::{AsRawFd, OsStrExt};
use std::ptr;
use std::slice;
use std::time::Duration;

/// Check if the file has an event available to read.
///
/// For gpiochip files the events are LineInfoChangeEvent.
/// For line request files the events are LineEdgeEvent.
#[inline]
pub fn has_event(f: &File) -> Result<bool> {
    wait_event(f, Duration::ZERO)
}

// workaround musl libc::ioctl() having a different signature
#[cfg(target_env = "musl")]
pub(crate) type IoctlRequestType = ::libc::c_int;
#[cfg(not(target_env = "musl"))]
pub(crate) type IoctlRequestType = ::libc::c_ulong;

macro_rules! ior {
    ($nr:expr, $dty:ty) => {
        ioctl_sys::ior!(IOCTL_MAGIC, $nr, std::mem::size_of::<$dty>()) as IoctlRequestType
    };
}

macro_rules! iorw {
    ($nr:expr, $dty:ty) => {
        ioctl_sys::iorw!(IOCTL_MAGIC, $nr, std::mem::size_of::<$dty>()) as IoctlRequestType
    };
}
pub(crate) use iorw;

/// Read an event from a chip or request file descriptor.
///
/// Returns the number of u64 words read.
#[inline]
pub fn read_event(f: &File, buf: &mut [u64]) -> Result<usize> {
    unsafe {
        let bufptr: *mut libc::c_void = std::ptr::addr_of_mut!(*buf) as *mut libc::c_void;
        match libc::read(f.as_raw_fd(), bufptr, buf.len() * 8) {
            -1 => Err(Error::from_errno()),
            x => {
                let size: usize = x.try_into().unwrap();
                if size % 8 == 0 {
                    Ok(size / 8)
                } else {
                    Err(Error::from(UnderReadError::new(
                        "read_event",
                        buf.len() * 8,
                        size,
                    )))
                }
            }
        }
    }
}

/// Wait for the file to have an event available to read.
pub fn wait_event(f: &File, d: Duration) -> Result<bool> {
    let mut pfd = libc::pollfd {
        fd: f.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // prevent musl builds complaining about use of deprecated time_t
    #[cfg(not(target_env = "musl"))]
    use libc::time_t as TimeT;
    #[cfg(all(target_env = "musl", target_pointer_width = "32"))]
    use std::primitive::i32 as TimeT;
    #[cfg(all(target_env = "musl", target_pointer_width = "64"))]
    use std::primitive::i64 as TimeT;
    let timeout = libc::timespec {
        tv_sec: d.as_secs() as TimeT,
        tv_nsec: d.subsec_nanos() as libc::c_long,
    };
    unsafe {
        match libc::ppoll(
            std::ptr::addr_of_mut!(pfd),
            1,
            std::ptr::addr_of!(timeout),
            ptr::null(),
        ) {
            -1 => Err(Error::from_errno()),
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
/// * `cf` - The open gpiochip device file.
pub fn get_chip_info(cf: &File) -> Result<ChipInfo> {
    let mut chip = std::mem::MaybeUninit::<ChipInfo>::uninit();
    unsafe {
        match libc::ioctl(
            cf.as_raw_fd(),
            ior!(Ioctl::GetChipInfo, ChipInfo),
            chip.as_mut_ptr(),
        ) {
            0 => Ok(chip.assume_init()),
            _ => Err(Error::from_errno()),
        }
    }
}

/// Remove any watch on changes to the [`LineInfo`] for a line.
///
/// * `cf` - The open gpiochip device file.
/// * `offset` - The offset of the line to unwatch.
///
/// [`LineInfo`]: struct.LineInfo.html
pub fn unwatch_line_info(cf: &File, offset: Offset) -> Result<()> {
    match unsafe { libc::ioctl(cf.as_raw_fd(), iorw!(Ioctl::UnwatchLineInfo, u32), &offset) } {
        0 => Ok(()),
        _ => Err(Error::from_errno()),
    }
}

/// An error number returned by a kernel ioctl call.
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub struct Errno(pub i32);

impl From<&std::io::Error> for Errno {
    fn from(e: &std::io::Error) -> Self {
        Errno(e.raw_os_error().unwrap_or(0))
    }
}
impl std::fmt::Display for Errno {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", std::io::Error::from_raw_os_error(self.0))
    }
}

/// The result returned by [`gpiocdev_uapi`] functions.
///
/// [`gpiocdev_uapi`]: crate
pub type Result<T> = std::result::Result<T, Error>;

/// Result returned by struct validators.
pub type ValidationResult = std::result::Result<(), ValidationError>;

/// Errors returned by [`gpiocdev_uapi`] functions.
///
/// [`gpiocdev_uapi`]: crate
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum Error {
    /// An error returned from an underlying system call.
    #[error(transparent)]
    Os(Errno),

    /// An error indicating insufficient data read for the expected object.
    #[error(transparent)]
    UnderRead(#[from] UnderReadError),

    /// An error validating an data structure retuned from the kernel
    #[error(transparent)]
    Validation(#[from] ValidationError),
}

impl Error {
    /// Create an error from the current errno value.
    #[inline]
    pub fn from_errno() -> Error {
        Error::Os(Errno(unsafe { *libc::__errno_location() }))
    }
}

/// A failure to read sufficient bytes to construct an object.
//
// This should never happen - but is checked to be safe.
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
#[error("Reading {obj} returned {found} bytes, expected {expected}.")]
pub struct UnderReadError {
    /// The struct that under read.
    pub obj: &'static str,
    /// The number of bytes expected.
    pub expected: usize,
    /// The number of bytes read.
    pub found: usize,
}

impl UnderReadError {
    /// Create an UnderReadError.
    pub(crate) fn new(obj: &'static str, expected: usize, found: usize) -> UnderReadError {
        UnderReadError {
            obj,
            expected,
            found,
        }
    }
}

/// A failure to validate a struct returned from a system call.
//
// Should only be seen if a kernel update adds an enum value we are unaware of.
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
#[error("Kernel returned invalid {field}: {msg}")]
pub struct ValidationError {
    /// The field that failed to validate.
    pub field: String,
    /// The details of the validation failure.
    pub msg: String,
}

impl ValidationError {
    /// Create a ValidationError.
    pub(crate) fn new<S: Into<String>, T: Into<String>>(field: S, msg: T) -> ValidationError {
        ValidationError {
            field: field.into(),
            msg: msg.into(),
        }
    }
}

/// The maximum number of bytes stored in a Name.
pub const NAME_LEN_MAX: usize = 32;

/// A uAPI name string, common to ABI v1 and v2.
#[repr(C)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Name([u8; NAME_LEN_MAX]);

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
    #[inline]
    pub fn as_os_str(&self) -> &OsStr {
        unsafe { OsStr::from_bytes(slice::from_raw_parts(&self.0[0], self.strlen())) }
    }

    /// Construct a Name from byte slice.
    ///
    /// Slice will be truncated if longer than the Name size.
    /// Truncation occurs on UTF-8 codepoint boundaries so the resulting
    /// name is still valid UTF-8.
    pub fn from_bytes(s: &[u8]) -> Name {
        let mut d: Name = Default::default();
        // drop any truncated UTF-8 codepoint
        let len = if s.len() < NAME_LEN_MAX {
            s.len()
        } else if s[NAME_LEN_MAX - 3] >= 0xf0 {
            NAME_LEN_MAX - 3
        } else if s[NAME_LEN_MAX - 2] >= 0xe0 {
            NAME_LEN_MAX - 2
        } else if s[NAME_LEN_MAX - 1] >= 0xc0 {
            NAME_LEN_MAX - 1
        } else {
            NAME_LEN_MAX
        };
        for (src, dst) in s.iter().take(len).zip(d.0.iter_mut()) {
            *dst = *src;
        }
        d
    }
}
impl From<&Name> for String {
    fn from(s: &Name) -> Self {
        String::from(s.as_os_str().to_string_lossy())
    }
}
impl From<&str> for Name {
    fn from(s: &str) -> Self {
        Name::from_bytes(s.as_bytes())
    }
}

/// An identifier for a line on a particular chip.
///
/// Valid offsets are in the range 0..`num_lines` as reported in the [`ChipInfo`].
pub type Offset = u32;

/// The maximum number of lines that may be requested in a single request.
pub const NUM_LINES_MAX: usize = 64;

/// A collection of line offsets.
///
/// Typically used to identify the lines belonging to a particular request.
#[repr(C)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Offsets([Offset; NUM_LINES_MAX]);

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

    /// Copy offsets from an iterable list.
    pub fn copy_from_slice(&mut self, s: &[u32]) {
        let extent = std::cmp::min(NUM_LINES_MAX, s.len());
        self.0[0..extent].copy_from_slice(s);
    }
}

impl Default for Offsets {
    fn default() -> Self {
        Offsets([0; NUM_LINES_MAX])
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
pub enum LineInfoChangeKind {
    /// The line has been requested.
    Requested = 1,

    /// The line has been released.
    Released = 2,

    /// The line has been reconfigured.
    Reconfigured = 3,
}

impl TryFrom<u32> for LineInfoChangeKind {
    type Error = String;

    fn try_from(v: u32) -> std::result::Result<Self, Self::Error> {
        Ok(match v {
            x if x == LineInfoChangeKind::Requested as u32 => LineInfoChangeKind::Requested,
            x if x == LineInfoChangeKind::Released as u32 => LineInfoChangeKind::Released,
            x if x == LineInfoChangeKind::Reconfigured as u32 => LineInfoChangeKind::Reconfigured,
            x => return Err(format!("invalid value: {}", x)),
        })
    }
}

impl LineInfoChangeKind {
    /// Confirm that the value read from the kernel is valid in Rust.
    pub(crate) fn validate(&self) -> std::result::Result<(), String> {
        LineInfoChangeKind::try_from(*self as u32).map(|_i| ())
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
        Ok(match v {
            x if x == LineEdgeEventKind::RisingEdge as u32 => LineEdgeEventKind::RisingEdge,
            x if x == LineEdgeEventKind::FallingEdge as u32 => LineEdgeEventKind::FallingEdge,
            _ => return Err(format!("invalid value: {}", v)),
        })
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
    fn size_of_chip_info() {
        assert_eq!(
            std::mem::size_of::<ChipInfo>(),
            68usize,
            concat!("Size of: ", stringify!(ChipInfo))
        );
    }

    #[test]
    fn line_info_changed_kind_validate() {
        let mut a = LineInfoChangeKind::Requested;
        assert!(a.validate().is_ok());
        unsafe {
            a = *(&0 as *const i32 as *const LineInfoChangeKind);
            assert_eq!(a.validate().unwrap_err(), "invalid value: 0");
            a = *(&4 as *const i32 as *const LineInfoChangeKind);
            assert_eq!(a.validate().unwrap_err(), "invalid value: 4");
            a = *(&3 as *const i32 as *const LineInfoChangeKind);
            assert!(a.validate().is_ok());
        }
    }

    #[test]
    fn line_event_kind_validate() {
        let mut a = LineInfoChangeKind::Requested;
        assert!(a.validate().is_ok());
        unsafe {
            a = *(&0 as *const i32 as *const LineInfoChangeKind);
            assert_eq!(a.validate().unwrap_err(), "invalid value: 0");
            a = *(&4 as *const i32 as *const LineInfoChangeKind);
            assert_eq!(a.validate().unwrap_err(), "invalid value: 4");
            a = *(&3 as *const i32 as *const LineInfoChangeKind);
            assert!(a.validate().is_ok());
        }
    }

    #[test]
    fn name_from_str() {
        let mut x = [0u8; 32];
        x[0] = 98;
        x[1] = 97;
        x[2] = 110;
        x[3] = 97;
        x[4] = 110;
        x[5] = 97;
        let mut a: Name = "banana".into();
        assert_eq!(a.0, x);

        a = "apple".into();
        x[0] = 97;
        x[1] = 112;
        x[2] = 112;
        x[3] = 108;
        x[4] = 101;
        x[5] = 0;
        assert_eq!(a.0, x);

        a = "an overly long truncated name -><- cut here".into();
        assert_eq!(a.as_os_str(), "an overly long truncated name ->");
    }

    #[test]
    fn name_is_empty() {
        let mut a = Name::default();
        assert!(a.is_empty());
        a = "banana".into();
        assert!(!a.is_empty());
    }

    #[test]
    fn name_strlen() {
        let mut a = Name::default();
        assert_eq!(a.strlen(), 0);
        a = "banana".into();
        assert_eq!(a.strlen(), 6);
        a = "an overly long truncated name -><- cut here".into();
        assert_eq!(a.strlen(), 32);
    }

    #[test]
    fn name_as_os_str() {
        let mut a = Name::default();
        assert_eq!(a.as_os_str(), "");
        a = "banana".into();
        assert_eq!(a.as_os_str(), "banana");
    }

    #[test]
    fn name_from_bytes() {
        // empty
        let mut a = Name::from_bytes("some bytes".as_bytes());
        assert_eq!(a.as_os_str(), "some bytes");

        // word
        a = Name::from_bytes("banana".as_bytes());
        assert_eq!(a.as_os_str(), "banana");

        // multiple words
        let mut a = Name::from_bytes("some bytes".as_bytes());
        assert_eq!(a.as_os_str(), "some bytes");

        // truncated overlength
        a = Name::from_bytes("an overly long truncated name -><- cut here".as_bytes());
        assert_eq!(a.as_os_str(), "an overly long truncated name ->");

        // truncated at UTF-8 code point - dangling 1 of 2
        a = Name::from_bytes("an overly long truncated name->ó<- cut here".as_bytes());
        assert_eq!(a.as_os_str(), "an overly long truncated name->");

        // truncated at UTF-8 code point - trailing 2 byte
        a = Name::from_bytes("an overly long truncated name ó<- cut here".as_bytes());
        assert_eq!(a.as_os_str(), "an overly long truncated name ó");

        // truncated at UTF-8 code point - dangling 1 of 3
        a = Name::from_bytes("an overly long truncated name->€<- cut here".as_bytes());
        assert_eq!(a.as_os_str(), "an overly long truncated name->");

        // truncated at UTF-8 code point - dangling 2 of 3
        a = Name::from_bytes("an overly long truncated name>€<- cut here".as_bytes());
        assert_eq!(a.as_os_str(), "an overly long truncated name>");

        // truncated at UTF-8 code point - trailing 3 byte
        a = Name::from_bytes("overly long truncated name - €<- cut here".as_bytes());
        assert_eq!(a.as_os_str(), "overly long truncated name - €");

        // truncated at UTF-8 code point - dangling 1 of 4
        a = Name::from_bytes("an overly long truncated name->𝄞<- cut here".as_bytes());
        assert_eq!(a.as_os_str(), "an overly long truncated name->");

        // truncated at UTF-8 code point - dangling 2 of 4
        a = Name::from_bytes("an overly long truncated name>𝄞<- cut here".as_bytes());
        assert_eq!(a.as_os_str(), "an overly long truncated name>");

        // truncated at UTF-8 code point - dangling 3 of 4
        a = Name::from_bytes("overly long truncated name ->𝄞<- cut here".as_bytes());
        assert_eq!(a.as_os_str(), "overly long truncated name ->");

        // truncated at UTF-8 code point - trailing 4 byte
        a = Name::from_bytes("overly long truncated name -𝄞<- cut here".as_bytes());
        assert_eq!(a.as_os_str(), "overly long truncated name -𝄞");
    }

    #[test]
    fn name_default() {
        assert_eq!(Name::default().0, [0u8; NAME_LEN_MAX]);
    }

    #[test]
    fn offsets_from_slice() {
        let mut x = [0u32; NUM_LINES_MAX];
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
    fn offsets_default() {
        assert_eq!(Offsets::default().0, [0u32; NUM_LINES_MAX]);
    }

    #[test]
    fn padding_is_zeroed() {
        let mut padding: Padding<3> = Padding::default();
        assert!(padding.is_zeroed());
        padding.0[1] = 3;
        assert!(!padding.is_zeroed());
    }

    #[test]
    fn size_of_name() {
        assert_eq!(
            std::mem::size_of::<Name>(),
            NAME_LEN_MAX,
            concat!("Size of: ", stringify!(Name))
        );
    }

    #[test]
    fn size_of_offsets() {
        assert_eq!(
            std::mem::size_of::<Offsets>(),
            256usize,
            concat!("Size of: ", stringify!(Offsets))
        );
    }

    #[test]
    fn size_of_padding() {
        assert_eq!(
            std::mem::size_of::<Padding<1>>(),
            4usize,
            concat!("Size of: ", stringify!(Padding<1>))
        );
        assert_eq!(
            std::mem::size_of::<Padding<2>>(),
            8usize,
            concat!("Size of: ", stringify!(Padding<2>))
        );
        assert_eq!(
            std::mem::size_of::<Padding<5>>(),
            20usize,
            concat!("Size of: ", stringify!(Padding<5>))
        );
    }
}
