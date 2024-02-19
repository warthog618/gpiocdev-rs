// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A library for accessing GPIO lines on Linux platforms
//! using the GPIO character device.
//!
//! Lines are requested and manipulated using the [`request`] module.
//!
//! The lines available on specific chips can be discovered using the [`chip`] module.
//!
//! The lines available on the system can be discovered by name using the [`find_named_line`]
//! and [`find_named_lines`] functions, or using the iterator returned by [`lines`].
//!
//! # Example Usage
//!
//! Request an input line and output line, and read from the input and change the output to that value:
//! ```no_run
//! # fn example() -> Result<(), gpiocdev::Error> {
//! use gpiocdev::line::{Bias, Value};
//!
//! let req = gpiocdev::Request::builder()
//!     .on_chip("/dev/gpiochip0")
//!     .with_line(3)
//!     .as_input()
//!     .with_bias(Bias::PullUp)
//!     .with_line(4)
//!     .as_output(Value::Inactive)
//!     .request()?;
//! let value = req.value(3)?;
//! req.set_value(4, value)?;
//! # Ok(())
//! # }
//! ```
//! Monitor a line for debounced edges:
//! ```no_run
//! # fn example() -> Result<(), gpiocdev::Error> {
//! let req = gpiocdev::Request::builder()
//!     .on_chip("/dev/gpiochip0")
//!     .with_line(5)
//!     .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
//!     .with_debounce_period(std::time::Duration::from_millis(5))
//!     .request()?;
//! for edge in req.edge_events() {
//!     println!("{:?}", edge);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! [`chip`]: module@chip
//! [`request`]: module@request

#[cfg(not(any(feature = "uapi_v1", feature = "uapi_v2")))]
compile_error!("Either feature \"uapi_v1\" or \"uapi_v2\" must be enabled for this crate.");

#[cfg(any(feature = "uapi_v1", feature = "uapi_v2"))]
use gpiocdev_uapi as uapi;
#[cfg(feature = "serde")]
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::ops::Range;
use std::path::PathBuf;

/// Types and functions specific to chips.
pub mod chip;
pub use chip::Chip;

/// Types specific to lines.
pub mod line;

/// Wrappers for various async reactors.
#[cfg(any(feature = "async_tokio", feature = "async_io"))]
mod r#async;

#[cfg(feature = "async_io")]
pub use r#async::async_io;
#[cfg(feature = "async_tokio")]
pub use r#async::tokio;

/// An iterator over all the GPIO lines visible to the caller.
pub fn lines() -> Result<LineIterator> {
    LineIterator::new()
}

/// Find the chip hosting a named line, and the line offset on that chip.
///
/// Stops at the first matching line, if one can be found.
///
/// Returns the path of the chip containing the line, and the offset of the line on that chip.
///
/// If checking that the line name is unique is required then use [`find_named_lines`]
/// with the strict option.
///
/// If multiple lines are required then [`find_named_lines`] is more performant.
///
/// # Examples
/// The found line can be used to request the line:
/// ```no_run
/// # fn example() -> Result<(), gpiocdev::Error> {
/// # use gpiocdev::line::Value;
/// let led0 = gpiocdev::find_named_line("LED0").unwrap();
/// let req = gpiocdev::Request::builder()
///     .with_found_line(&led0)
///     .as_output(Value::Active)
///     .request()?;
/// # Ok(())
/// # }
/// ```
///
/// Using the chip and offset from the found line to request the line:
/// ```no_run
/// # fn example() -> Result<(), gpiocdev::Error> {
/// # use gpiocdev::line::Value;
/// let led0 = gpiocdev::find_named_line("LED0").unwrap();
/// let req = gpiocdev::Request::builder()
///     .on_chip(&led0.chip)
///     .with_line(led0.info.offset)
///     .as_output(Value::Active)
///     .request()?;
/// # Ok(())
/// # }
/// ```
pub fn find_named_line(name: &str) -> Option<FoundLine> {
    if let Ok(mut liter) = LineIterator::new() {
        return liter.find(|l| l.info.name == name);
    }
    None
}

/// Find a collection of named lines.
///
///  - `strict`: if true then the names are checked to be unique within the available lines
///
/// For each name, returns the first matching line, if one can be found.
/// If it cannot be found then there will be no matching entry in the returned map.
///
/// Returns the path of the chip containing the line, the offset of the line on that chip,
/// and the info for the line.
///
/// # Examples
/// Adding the found lines to the request directly:
/// ```no_run
/// # fn example() -> Result<(), gpiocdev::Error> {
/// # use gpiocdev::line::Value;
/// let sensors = gpiocdev::find_named_lines(&["SENSOR0", "SENSOR1"], true)?;
/// let req = gpiocdev::Request::builder()
///     .with_found_lines(&sensors)
///     .as_input()
///     .request()?;
/// let sensor1 = sensors.get("SENSOR1").unwrap();
/// let value = req.value(sensor1.info.offset)?;
/// # Ok(())
/// # }
///```
///
/// Using the individual found lines to request the lines with different
/// configuration for each line:
/// ```no_run
/// # fn example() -> Result<(), gpiocdev::Error> {
/// # use gpiocdev::line::Value;
/// let lines = gpiocdev::find_named_lines(&["SENSOR0", "LED0"], true)?;
/// let sensor0 = lines.get("SENSOR0").unwrap();
/// let led0 = lines.get("LED0").unwrap();
/// let req = gpiocdev::Request::builder()
///     .with_found_line(&sensor0)
///     .as_input()
///     .with_found_line(&led0)
///     .as_output(Value::Active)
///     .request()?;
/// let value = req.value(sensor0.info.offset)?;
/// # Ok(())
/// # }
/// ```
///
pub fn find_named_lines<'a>(
    names: &'a [&'a str],
    strict: bool,
) -> Result<HashMap<&'a str, FoundLine>> {
    let mut found = HashMap::new();
    for l in LineIterator::new()? {
        for name in names {
            if *name != l.info.name.as_str() {
                continue;
            }
            if !found.contains_key(*name) {
                found.insert(name.to_owned(), l.clone());
                if !strict && found.len() == names.len() {
                    return Ok(found);
                }
            } else if strict {
                return Err(Error::NonuniqueLineName(name.to_string()));
            }
            // else already have that line...
        }
    }
    Ok(found)
}

/// The info for a line discovered in the system.
///
/// Identifies the chip hosting the line, and the line info.
///
/// The discovered line can be used to request the line:
/// ```no_run
/// # fn example() -> Result<(), gpiocdev::Error> {
/// # use gpiocdev::line::Value;
/// let led0 = gpiocdev::find_named_line("LED0").unwrap();
/// let req = gpiocdev::Request::builder()
///     .with_found_line(&led0)
///     .as_output(Value::Active)
///     .request()?;
/// # Ok(())
/// # }
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FoundLine {
    /// The path to the chip containing the line.
    pub chip: PathBuf,
    /// The info of the line.
    pub info: line::Info,
}

#[cfg(test)]
impl From<line::Offset> for FoundLine {
    fn from(offset: line::Offset) -> Self {
        let mut f = FoundLine::default();
        f.info.offset = offset;
        f
    }
}

/// An iterator for all lines in the system available to the caller.
///
/// Can be used to discover and filter lines based on by particular criteria.
///
/// Used by [`find_named_line`] and [`find_named_lines`] to find lines based on line name.
/// ```no_run
/// # fn example() -> Result<(), gpiocdev::Error> {
/// # use gpiocdev::line::Value;
/// // replicating find_named_line...
/// let led2 = gpiocdev::LineIterator::new()?.find(|l| l.info.name == "LED2").unwrap();
/// let req = gpiocdev::Request::builder()
///     .with_found_line(&led2)
///     .as_output(Value::Active)
///     .request()?;
/// # Ok(())
/// # }
/// ```
pub struct LineIterator {
    chips: Vec<PathBuf>,
    citer: Range<usize>,
    chip: chip::Chip,
    liter: Range<u32>,
}

fn next_chip(chips: &[PathBuf], citer: &mut Range<usize>) -> Option<(chip::Chip, Range<u32>)> {
    for cidx in citer {
        if let Ok(chip) = chip::Chip::from_path(&chips[cidx]) {
            if let Ok(info) = chip.info() {
                return Some((
                    chip,
                    Range {
                        start: 0,
                        end: info.num_lines,
                    },
                ));
            }
        }
    }
    None
}

impl LineIterator {
    /// Creates an iterator over all the GPIO lines in the system that are available to the caller.
    pub fn new() -> Result<Self> {
        let chips = chip::chips()?;
        let mut citer = Range {
            start: 0,
            end: chips.len(),
        };
        if let Some((chip, liter)) = next_chip(&chips, &mut citer) {
            Ok(LineIterator {
                chips,
                citer,
                chip,
                liter,
            })
        } else {
            Err(Error::NoGpioChips())
        }
    }

    fn next_line_info(&mut self) -> Option<line::Info> {
        for offset in &mut self.liter {
            if let Ok(linfo) = self.chip.line_info(offset) {
                return Some(linfo);
            }
        }
        None
    }
}

impl Iterator for LineIterator {
    type Item = FoundLine;

    fn next(&mut self) -> Option<FoundLine> {
        if let Some(linfo) = self.next_line_info() {
            return Some(FoundLine {
                chip: self.chip.path().to_path_buf(),
                info: linfo,
            });
        }
        if let Some((chip, liter)) = next_chip(&self.chips, &mut self.citer) {
            self.chip = chip;
            self.liter = liter;
            if let Some(linfo) = self.next_line_info() {
                return Some(FoundLine {
                    chip: self.chip.path().to_path_buf(),
                    info: linfo,
                });
            }
        }
        None
    }
}

/// Types and functions related to requesting lines.
///
/// The [`Builder`] creates the [`Request`] which can access and manipulate sets of lines
/// and return edge events via the [`EdgeEventBuffer`].
///
/// The line configuration is provided to the [`Builder`] using either direct mutators,
/// such as [`as_input`] and [`with_edge_detection`], or via a [`Config`].
///
/// # Examples
/// Request and read a basic input line:
/// ```no_run
/// # fn example() -> Result<(), gpiocdev::Error> {
/// let l3 = gpiocdev::Request::builder()
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
pub use request::Request;

/// The uAPI ABI versions available to interact with the kernel.
///
/// Two versions of the Linux GPIO uAPI ABI currently exist, with v1 being released in
/// Linux 4.8 and v2 being released in Linux 5.10.
/// Both versions of the uAPI ABI are supported by `gpiocdev`, both individually and combined,
/// with the library defaulting to v2.
///
/// The v1 uAPI can be selected as a feature for applications where v2 may be unavailable.
///
/// * `V2` is the current ABI and is used by default.
/// * `V1` is more restrictive than V2, so some information and features are
/// unavailable.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum AbiVersion {
    /// GPIO uAPI v1
    V1,
    /// GPIO uAPI v2
    #[default]
    V2,
}

impl fmt::Display for AbiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AbiVersion::V1 => write!(f, "uAPI ABI v1"),
            AbiVersion::V2 => write!(f, "uAPI ABI v2"),
        }
    }
}

/// Errors returned by [`gpiocdev`] functions.
///
/// [`gpiocdev`]: crate
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum Error {
    /// An operation cannot be performed due to a limitation in the ABI version being used.
    #[error("{0} {1}.")]
    AbiLimitation(AbiVersion, String),

    /// Problem accessing GPIO chip character devices
    #[error("\"{0}\" {1}.")]
    GpioChip(PathBuf, chip::ErrorKind),

    /// An error returned when there is a problem with an argument.
    #[error("{0}")]
    InvalidArgument(String),

    /// No gpiochips are available to the user.
    #[error("No GPIO chips are available")]
    NoGpioChips(),

    /// Returned when the strict mode of [`find_named_lines`] finds multiple lines with the same name.
    #[error("Line name '{0}' is not unique")]
    NonuniqueLineName(String),

    /// An error returned from an underlying os call.
    #[error(transparent)]
    Os(uapi::Errno),

    /// An error returned from an underlying uAPI call.
    #[error("uAPI {0} returned: {1}")]
    Uapi(UapiCall, #[source] uapi::Error),

    /// The response to a uAPI command contained unexpected content.
    #[error("{0}")]
    UnexpectedResponse(String),

    /// The kernel or build does not support the requested uAPI ABI version.
    #[error("{0} is not supported by the {1}.")]
    UnsupportedAbi(AbiVersion, AbiSupportKind),

    /// The kernel has no support for any uAPI ABI version.
    #[error("uAPI ABI is not supported by the kernel.")]
    NoAbiSupport(),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Os(uapi::Errno::from(&e))
    }
}

/// Identifiers for the underlying uAPI calls.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
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
    ReadEvent,
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
            UapiCall::ReadEvent => "read_event",
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AbiSupportKind {
    /// The library does not have the feature enabled for the requested ABI version.
    Build,

    /// The kernel running on the platform does not support the requested ABI version.
    #[default]
    Kernel,
}

impl fmt::Display for AbiSupportKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AbiSupportKind::Build => write!(f, "build"),
            AbiSupportKind::Kernel => write!(f, "kernel"),
        }
    }
}

/// The result for [`gpiocdev`] functions.
///
/// [`gpiocdev`]: crate
pub type Result<T> = std::result::Result<T, Error>;

/// Detect the most recent uAPI ABI supported by the platform.
pub fn detect_abi_version() -> Result<AbiVersion> {
    for p in chip::chips()? {
        if let Ok(c) = chip::Chip::from_path(p) {
            return c.detect_abi_version();
        }
    }
    Err(Error::NoGpioChips())
}

/// Check if the platform and library support a specific ABI version.
pub fn supports_abi_version(abiv: AbiVersion) -> Result<()> {
    for p in chip::chips()? {
        if let Ok(c) = chip::Chip::from_path(p) {
            return c.supports_abi_version(abiv);
        }
    }
    Err(Error::NoGpioChips())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod uapi_call {

        #[test]
        fn display() {
            use super::UapiCall;
            let uc = UapiCall::GetChipInfo;
            assert_eq!(format!("{}", uc), "get_chip_info");
            let uc = UapiCall::GetLine;
            assert_eq!(format!("{}", uc), "get_line");
            let uc = UapiCall::GetLineEvent;
            assert_eq!(format!("{}", uc), "get_line_event");
            let uc = UapiCall::GetLineHandle;
            assert_eq!(format!("{}", uc), "get_line_handle");
            let uc = UapiCall::GetLineInfo;
            assert_eq!(format!("{}", uc), "get_line_info");
            let uc = UapiCall::GetLineValues;
            assert_eq!(format!("{}", uc), "get_line_values");
            let uc = UapiCall::HasEvent;
            assert_eq!(format!("{}", uc), "has_event");
            let uc = UapiCall::LEEFromBuf;
            assert_eq!(format!("{}", uc), "LineEdgeEvent::from_buf");
            let uc = UapiCall::LICEFromBuf;
            assert_eq!(format!("{}", uc), "LineInfoChangeEvent::from_buf");
            let uc = UapiCall::ReadEvent;
            assert_eq!(format!("{}", uc), "read_event");
            let uc = UapiCall::SetLineConfig;
            assert_eq!(format!("{}", uc), "set_line_config");
            let uc = UapiCall::SetLineValues;
            assert_eq!(format!("{}", uc), "set_line_values");
            let uc = UapiCall::WaitEvent;
            assert_eq!(format!("{}", uc), "wait_event");
            let uc = UapiCall::WatchLineInfo;
            assert_eq!(format!("{}", uc), "watch_line_info");
            let uc = UapiCall::UnwatchLineInfo;
            assert_eq!(format!("{}", uc), "unwatch_line_info");
        }
    }

    mod abi_support_kind {

        #[test]
        fn display() {
            use crate::AbiSupportKind;
            let ask = AbiSupportKind::Build;
            assert_eq!(format!("{}", ask), "build");
            let ask = AbiSupportKind::Kernel;
            assert_eq!(format!("{}", ask), "kernel");
        }
    }
}
