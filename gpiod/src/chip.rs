// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::line::Offset;
use crate::{
    line, line::InfoChangeEvent, AbiSupportKind, AbiVersion, AbiVersion::*, Error, Name, Result,
    UapiCall,
};
#[cfg(all(feature = "uapi_v1", not(feature = "uapi_v2")))]
use gpiod_uapi::v1 as uapi;
#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
use gpiod_uapi::v2 as uapi;
#[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
use gpiod_uapi::{v1, v2};
use std::fmt;
use std::fs;
use std::mem::size_of;
use std::os::linux::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

const CHARDEV_MODE: u32 = 0x2000;

/// Check if a path corresponds to a GPIO character device.
///
/// Returns the resolved path to the character device.
pub fn is_chip<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    let pb = fs::canonicalize(&path)?;
    let m = fs::symlink_metadata(&pb)?;
    if m.st_mode() & CHARDEV_MODE == 0 {
        return Err(Error::GpioChip(pb, ErrorKind::NotCharacterDevice));
    }
    let mut sysfs_dev = PathBuf::from("/sys/bus/gpio/devices");
    sysfs_dev.push(pb.file_name().unwrap());
    sysfs_dev.push("dev");
    if let Ok(rdev) = fs::read_to_string(sysfs_dev) {
        let st_rdev = m.st_rdev();
        let dev_str = format!("{}:{}", (st_rdev as u16 >> 8) as u8, st_rdev as u8);
        if rdev.trim_end() == dev_str {
            return Ok(pb);
        }
    }
    Err(Error::GpioChip(pb, ErrorKind::NotGpioDevice))
}

/// Returns the paths of all the GPIO character devices on the system.
///
// This is expected to be small, so return a vec, rather than using an iterator.
pub fn chips() -> Result<Vec<PathBuf>> {
    let mut chips = Vec::new();
    let rd = std::fs::read_dir("/dev")?;
    for de in rd.flatten() {
        let path = de.path();
        if let Ok(dp) = is_chip(&path) {
            if !chips.contains(&dp) {
                chips.push(dp);
            }
        }
    }
    chips.sort();
    Ok(chips)
}

/// A GPIO character device.
pub struct Chip {
    /// The resolved path of the GPIO character device.
    path: PathBuf,
    pub(crate) f: fs::File,
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    abiv: AbiVersion,
}

impl Chip {
    /// Constructs a Chip using the given path.
    ///
    /// The path must resolve to a valid GPIO character device.
    pub fn from_path<P: AsRef<Path>>(p: P) -> Result<Chip> {
        let path = is_chip(p.as_ref())?;
        let f = fs::File::open(&path)?;
        Ok(Chip {
            path,
            f,
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            abiv: V2,
        })
    }
    /// Get the information for the chip.
    pub fn info(&self) -> Result<Info> {
        Ok(Info::from(
            uapi::get_chip_info(&self.f).map_err(|e| Error::UapiError(UapiCall::GetChipInfo, e))?,
        ))
    }

    /// Return the name of the chip.
    ///
    /// This is based on the filename component of the resolved chip path, not the name
    /// from the [`Info`], so it does not involve any system calls.
    ///
    /// [`Info`]: Info
    pub fn name(&self) -> Name {
        // The unwrap can only fail for directories, and the path is known to refer to a file.
        Name::from(self.path.file_name().unwrap())
    }

    /// Return the path of the chip.
    pub fn path(&self) -> &Path {
        self.path.as_ref()
    }

    /// Find the offset of the named line.
    ///
    /// Returns the first matching line.
    pub fn find_line(&self, line: &str) -> Option<Offset> {
        if let Ok(ci) = self.info() {
            for offset in 0..ci.num_lines {
                if let Ok(li) = self.line_info(offset) {
                    if li.name.as_os_str() == line {
                        return Some(offset);
                    }
                }
            }
        }
        None
    }

    /// Get the information for a line on the chip.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    pub fn line_info(&self, offset: u32) -> Result<line::Info> {
        let res = match self.abiv {
            V1 => v1::get_line_info(&self.f, offset).map(|li| line::Info::from(&li)),
            V2 => v2::get_line_info(&self.f, offset).map(|li| line::Info::from(&li)),
        };
        res.map_err(|e| Error::UapiError(UapiCall::GetLineInfo, e))
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    pub fn line_info(&self, offset: u32) -> Result<line::Info> {
        uapi::get_line_info(&self.f, offset)
            .map(|li| line::Info::from(&li))
            .map_err(|e| Error::UapiError(UapiCall::GetLineInfo, e))
    }

    /// Add a watch for changes to the publicly available information on a line.
    ///
    /// This is a null operation if there is already a watch on the line.
    pub fn watch_line_info(&self, offset: u32) -> Result<line::Info> {
        self.do_watch_line_info(offset)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_watch_line_info(&self, offset: u32) -> Result<line::Info> {
        let res = match self.abiv {
            V1 => v1::watch_line_info(&self.f, offset).map(|li| line::Info::from(&li)),
            V2 => v2::watch_line_info(&self.f, offset).map(|li| line::Info::from(&li)),
        };
        res.map_err(|e| Error::UapiError(UapiCall::WatchLineInfo, e))
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_watch_line_info(&self, offset: u32) -> Result<line::Info> {
        uapi::watch_line_info(&self.f, offset)
            .map(|li| line::Info::from(&li))
            .map_err(|e| Error::UapiError(UapiCall::WatchLineInfo, e))
    }
    /// Remove a watch for changes to the publicly available information on a line.
    ///
    /// This is a null operation if there is no existing watch on the line.
    pub fn unwatch_line_info(&self, offset: u32) -> Result<()> {
        uapi::unwatch_line_info(&self.f, offset)
            .map_err(|e| Error::UapiError(UapiCall::UnwatchLineInfo, e))
    }

    /// Check if the request has at least one info change event available to read.
    pub fn has_line_info_change_event(&self) -> Result<bool> {
        gpiod_uapi::has_event(&self.f).map_err(|e| Error::UapiError(UapiCall::HasEvent, e))
    }

    /// Wait for an info change event to be available.
    pub fn wait_line_info_change_event(&self, timeout: Duration) -> Result<bool> {
        gpiod_uapi::wait_event(&self.f, timeout)
            .map_err(|e| Error::UapiError(UapiCall::WaitEvent, e))
    }

    /// Detect the most recent uAPI ABI supported by the library for the chip.
    pub fn detect_abi_version(&self) -> Result<AbiVersion> {
        // check in preferred order
        for abiv in [V2, V1] {
            if self.supports_abi_version(abiv).is_ok() {
                return Ok(abiv);
            }
        }
        Err(Error::UnsupportedAbi(
            AbiVersion::V2,
            AbiSupportKind::Platform,
        ))
    }
    /// Check if the platform and library support a specific ABI version.
    pub fn supports_abi_version(&self, abiv: AbiVersion) -> Result<()> {
        self.do_supports_abi_version(abiv)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_supports_abi_version(&self, abiv: AbiVersion) -> Result<()> {
        let res = match abiv {
            V1 => v1::get_line_info(&self.f, 0).map(|_| ()),
            V2 => v2::get_line_info(&self.f, 0).map(|_| ()),
        };
        res.map_err(|_| Error::UnsupportedAbi(abiv, AbiSupportKind::Platform))
    }
    #[cfg(all(feature = "uapi_v1", not(feature = "uapi_v2")))]
    fn do_supports_abi_version(&self, abiv: AbiVersion) -> Result<()> {
        match abiv {
            V1 => uapi::get_line_info(&self.f, 0)
                .map(|_| ())
                .map_err(|_| Error::UnsupportedAbi(abiv, AbiSupportKind::Platform)),
            V2 => Err(Error::UnsupportedAbi(
                AbiVersion::V2,
                AbiSupportKind::Library,
            )),
        }
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_supports_abi_version(&self, abiv: AbiVersion) -> Result<()> {
        match abiv {
            V2 => uapi::get_line_info(&self.f, 0)
                .map(|_| ())
                .map_err(|_| Error::UnsupportedAbi(abiv, AbiSupportKind::Platform)),
            V1 => Err(Error::UnsupportedAbi(
                AbiVersion::V1,
                AbiSupportKind::Library,
            )),
        }
    }

    /// Set the ABI version to use for subsequent operations.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    pub fn using_abi_version(&mut self, abiv: AbiVersion) -> &mut Self {
        self.abiv = abiv;
        self
    }

    /// Read an InfoChangeEvent from a buffer.
    ///
    /// Assumes the buffer has been previously populated by a call to read on the request.
    pub fn line_info_change_event_from_buf(&self, d: &[u8]) -> Result<InfoChangeEvent> {
        self.do_line_info_change_event_from_buf(d)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_line_info_change_event_from_buf(&self, d: &[u8]) -> Result<InfoChangeEvent> {
        match self.abiv {
            V1 => Ok(InfoChangeEvent::from(
                v1::LineInfoChangeEvent::from_buf(d)
                    .map_err(|e| Error::UapiError(UapiCall::LICEFromBuf, e))?,
            )),
            V2 => Ok(InfoChangeEvent::from(
                v2::LineInfoChangeEvent::from_buf(d)
                    .map_err(|e| Error::UapiError(UapiCall::LICEFromBuf, e))?,
            )),
        }
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_line_info_change_event_from_buf(&self, d: &[u8]) -> Result<InfoChangeEvent> {
        Ok(InfoChangeEvent::from(
            uapi::LineInfoChangeEvent::from_buf(d)
                .map_err(|e| Error::UapiError(UapiCall::LICEFromBuf, e))?,
        ))
    }

    /// The number of bytes required to buffer a single event read from the chip.
    ///
    /// This can be used to size a [u8] buffer to read events into - the buffer size
    /// should be a multiple of this size.
    pub fn line_info_change_event_size(&self) -> usize {
        self.do_line_info_change_event_size()
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_line_info_change_event_size(&self) -> usize {
        match self.abiv {
            V1 => size_of::<v1::LineInfoChangeEvent>(),
            V2 => size_of::<v2::LineInfoChangeEvent>(),
        }
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_line_info_change_event_size(&self) -> usize {
        size_of::<uapi::LineInfoChangeEvent>()
    }
}
impl std::io::Read for Chip {
    // This will read in v1::LineChangedEvent or v2:LineChangedEvent sized chunks
    // so buf must be at least as large as one event.
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.f.read(buf)
    }
}

/// The publicly available information for a GPIO chip.
pub struct Info {
    /// The system name for the chip, such as "*gpiochip0*".
    pub name: Name,
    /// A functional name for the chip.
    ///
    /// This typically identifies the type of GPIO chip.
    pub label: Name,
    /// The number of lines provided by the chip.
    pub num_lines: u32,
}
impl From<uapi::ChipInfo> for Info {
    fn from(ci: uapi::ChipInfo) -> Self {
        Info {
            name: Name::from(&ci.name),
            label: Name::from(&ci.label),
            num_lines: ci.num_lines,
        }
    }
}

/// Reasons a file cannot be opened as a GPIO character device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// File is not a character device.
    NotCharacterDevice,
    /// File is not a GPIO character device.
    NotGpioDevice,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            ErrorKind::NotCharacterDevice => "is not a character device",
            ErrorKind::NotGpioDevice => "is not a GPIO character device",
        };
        write!(f, "{}", msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(feature = "uapi_v1", not(feature = "uapi_v2")))]
    use gpiod_uapi::v1 as uapi;
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    use gpiod_uapi::v2 as uapi;
    use gpiod_uapi::Name;

    // Chip tests are all integration tests as construction requires GPIO chips.

    #[test]
    fn info_from_uapi() {
        let ui = uapi::ChipInfo {
            name: Name::from_bytes("banana".as_bytes()),
            label: Name::from_bytes("peel".as_bytes()),
            num_lines: 42,
        };
        let i = Info::from(ui);
        assert_eq!(i.num_lines, 42);
        assert_eq!(i.name.as_os_str(), "banana");
        assert_eq!(i.label.as_os_str(), "peel");
    }
}
