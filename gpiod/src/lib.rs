// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

//! A library for accessing GPIO lines on Linux platforms
//! using the GPIO character device.
//!
//! The lines available on chips can be discovered using the [`chip`] module.
//!
//! Lines can be requested and manipulated using the [`request`] module.
//!
//! [`chip`]: module@chip
//! [`request`]: module@request

#[cfg(not(any(feature = "uapi_v1", feature = "uapi_v2")))]
compile_error!("Either feature \"uapi_v1\" or \"uapi_v2\" must be enabled for this crate.");

#[cfg(any(feature = "uapi_v1", feature = "uapi_v2"))]
use gpiod_uapi as uapi;
use std::borrow::Cow;
use std::fmt;
use std::os::unix::prelude::OsStrExt;
use std::path::PathBuf;

/// Types and functions specific to chips.
pub mod chip;

/// Types specific to lines.
pub mod line;

/// Types and functions related to requesting lines.
///
/// The [`Builder`] creates the [`Request`] which can access and manipulate sets of lines,
/// and return edge events via the [`EdgeEventBuffer`].
///
/// The line configuration can be provided to the [`Builder`] using either direct mutators,
/// such as [`as_input`] and [`with_edge_detection`], or via a [`Config`].
///
/// To request and read a basic input line:
/// ```no_run
/// # use gpiod::Result;
/// use gpiod::request::Request;
///
/// # fn main() -> Result<()> {
/// let l3 = Request::builder()
///     .on_chip("/dev/gpiochip0")
///     .with_line(3)
///     .request()?;
/// let value = l3.value(3)?;
/// # Ok(())
/// # }
/// ```
///
/// [`Builder`]: struct.Builder.html
/// [`Config`]: struct.Config.html
/// [`EdgeEventBuffer`]: struct.EdgeEventBuffer.html
/// [`Request`]: struct.Request.html
/// [`as_input`]: struct.Builder.html#method.as_input
/// [`with_edge_detection`]: struct.Builder.html#method.with_edge_detection
pub mod request;

use chrono::{DateTime, TimeZone, Utc};
use std::ffi::{OsStr, OsString};

/// The uAPI ABI versions available to interact with the kernel.
///
/// Two versions of the Linux GPIO uAPI ABI currently exist, with v1 being released in
/// Linux v4.8 and v2 being released in Linux v5.10.
/// Both versions of the uAPI ABI are supported by `gpiod`, both individually and combined,
/// with the library defaulting to v2.
///
/// The v1 uAPI can be selected as a feature for applications where v2 may be unavailable.
///
/// * `V2` is the current ABI and is used by default.
/// * `V1` is more restrictive than V2, so some information and features are
/// unavailable, but the ABI itself is more widely available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbiVersion {
    V1,
    V2,
}

impl Default for AbiVersion {
    fn default() -> Self {
        Self::V2
    }
}

impl fmt::Display for AbiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AbiVersion::V1 => write!(f, "uAPI ABI v1"),
            AbiVersion::V2 => write!(f, "uAPI ABI v2"),
        }
    }
}

/// A name string.
///
/// Typically returned by the operating system.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Name(OsString);

impl Name {
    pub fn as_os_str(&self) -> &OsStr {
        self.0.as_os_str()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        self.0.to_string_lossy()
    }
}
impl From<&uapi::Name> for Name {
    fn from(n: &uapi::Name) -> Self {
        Name(n.as_os_str().to_os_string())
    }
}
impl From<Name> for uapi::Name {
    fn from(s: Name) -> Self {
        uapi::Name::from_bytes(s.as_os_str().as_bytes())
    }
}
impl From<&OsStr> for Name {
    fn from(n: &OsStr) -> Self {
        Name(n.to_os_string())
    }
}
impl From<&str> for Name {
    fn from(n: &str) -> Self {
        Name(n.to_string().into())
    }
}
impl From<Name> for OsString {
    fn from(n: Name) -> Self {
        n.0
    }
}

/// A moment in time in utc.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Create a Timestamp from the numner of nanoseconds.
    ///
    /// Suitable for  **CLOCK_REALTIME** clock sources.
    pub fn from_nanos(t: u64) -> Self {
        let sec = (t / 1000000000) as i64;
        let nsec = (t as u32) % 1000000000;
        Timestamp(Utc.timestamp(sec, nsec))
    }
}

impl From<Timestamp> for DateTime<Utc> {
    fn from(ts: Timestamp) -> Self {
        ts.0
    }
}

/// Errors returned by [`gpiod`] functions.
///
/// [`gpiod`]: crate
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An operation cannot be performed due to a limitation in the ABI version being used.
    #[error("{0} {1}.")]
    AbiLimitation(AbiVersion, String),
    /// An error returned when there is a problem with an argument.
    #[error("{0}")]
    InvalidArgument(String),
    /// No gpiochips are available to the user.
    #[error("No GPIO chips are available")]
    NoGpioChips(),
    /// Problem accessing GPIO chip character devices
    #[error("\"{0}\" {1}.")]
    GpioChip(PathBuf, chip::ErrorKind),
    /// An error returned from an underlying os call.
    #[error(transparent)]
    OsError(#[from] std::io::Error),
    /// An error returned from an underlying uAPI call.
    #[error("uAPI {0} returned: {1}")]
    UapiError(UapiCall, #[source] uapi::Error),
    /// The response to a uAPI command contained unexpected content.
    #[error("{0}")]
    UnexpectedResponse(String),
    /// The platform or library does not support the requested uAPI ABI version.
    #[error("{0} is not supported by the {1}.")]
    UnsupportedAbi(AbiVersion, AbiSupportKind),
}

/// Identifiers for the underlying uAPI calls.
#[doc(hidden)]
#[derive(Debug, Eq, PartialEq)]
pub enum UapiCall {
    GetChipInfo,
    GetLine,
    GetLineEvent,
    GetLineHandle,
    GetLineInfo,
    GetLineValues,
    HasEvent,
    LEEFromBuf,
    LICEFromBuf,
    SetLineConfig,
    SetLineValues,
    UnwatchLineInfo,
    WaitEvent,
    WatchLineInfo,
}

impl fmt::Display for UapiCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            UapiCall::GetChipInfo => "get_chip_info",
            UapiCall::GetLine => "get_line",
            UapiCall::GetLineEvent => "get_line_event",
            UapiCall::GetLineHandle => "get_line_handle",
            UapiCall::GetLineInfo => "get_line_info",
            UapiCall::GetLineValues => "get_line_values",
            UapiCall::HasEvent => "has_event",
            UapiCall::LEEFromBuf => "LineEdgeEvent::from_buf",
            UapiCall::LICEFromBuf => "LineInfoChangeEvent::from_buf",
            UapiCall::SetLineConfig => "set_line_config",
            UapiCall::SetLineValues => "set_line_values",
            UapiCall::UnwatchLineInfo => "unwatch_line_info",
            UapiCall::WaitEvent => "wait_event",
            UapiCall::WatchLineInfo => "watch_line_info",
        };
        write!(f, "{}", name)
    }
}

/// Components that may not support a particular ABI version.
#[doc(hidden)]
#[derive(Debug, Eq, PartialEq)]
pub enum AbiSupportKind {
    /// The library does not have the feature enabled for the requested ABI version.
    Library,
    /// The kernel runniong on the platform does not support the requested ABI version.
    Platform,
}

impl fmt::Display for AbiSupportKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AbiSupportKind::Library => write!(f, "library"),
            AbiSupportKind::Platform => write!(f, "platform"),
        }
    }
}

/// The result for [`gpiod`] functions.
///
/// [`gpiod`]: crate
pub type Result<T> = std::result::Result<T, Error>;

/// Detect the most recent uAPI ABI supported by the platform.
pub fn detect_abi_version() -> Result<AbiVersion> {
    for p in chip::chips()? {
        if let Ok(c) = chip::Chip::from_path(&p) {
            return c.detect_abi_version();
        }
    }
    Err(Error::NoGpioChips())
}

/// Check if the platform and library support a specific ABI version.
pub fn supports_abi_version(abiv: AbiVersion) -> Result<()> {
    for p in chip::chips()? {
        if let Ok(c) = chip::Chip::from_path(&p) {
            return c.supports_abi_version(abiv);
        }
    }
    Err(Error::NoGpioChips())
}
