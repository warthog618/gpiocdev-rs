// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::line::Offset;
use crate::{
    line, line::InfoChangeEvent, AbiSupportKind, AbiVersion, AbiVersion::*, Error, Result, UapiCall,
};
#[cfg(all(feature = "uapi_v1", not(feature = "uapi_v2")))]
use gpiocdev_uapi::v1 as uapi;
#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
use gpiocdev_uapi::v2 as uapi;
#[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
use gpiocdev_uapi::{v1, v2};
#[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
use std::cell::RefCell;
use std::fmt;
use std::fs;
use std::mem;
use std::ops::Range;
use std::os::linux::fs::MetadataExt;
use std::os::unix::prelude::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::time::Duration;

const CHARDEV_MODE: u32 = 0x2000;

/// Check if a path corresponds to a GPIO character device.
///
/// Returns the resolved path to the character device.
pub fn is_chip<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    let pb = fs::canonicalize(&path).map_err(crate::errno_from_ioerr)?;
    let m = fs::symlink_metadata(&pb).map_err(crate::errno_from_ioerr)?;
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
/// The returned paths are sorted in name order and are confirmed to be GPIO character devices,
/// so there is no need to check them with [`is_chip`].
pub fn chips() -> Result<Vec<PathBuf>> {
    let mut chips = std::fs::read_dir("/dev")
        .map_err(crate::errno_from_ioerr)?
        .filter_map(|x| x.ok())
        .flat_map(|de| is_chip(de.path()))
        .collect::<Vec<PathBuf>>();
    chips.sort_unstable_by(|a, b| human_sort::compare(&a.to_string_lossy(), &b.to_string_lossy()));
    chips.dedup();
    Ok(chips)
}

/// An iterator that returns the info for each line on the [`Chip`].
pub struct LineInfoIterator<'a> {
    chip: &'a Chip,
    offsets: Range<Offset>,
}

impl<'a> Iterator for LineInfoIterator<'a> {
    type Item = Result<line::Info>;

    fn next(&mut self) -> Option<Result<line::Info>> {
        match self.offsets.next() {
            Some(offset) => Some(self.chip.line_info(offset)),
            None => None,
        }
    }
}

/// A GPIO character device.
#[derive(Debug)]
pub struct Chip {
    /// The resolved path of the GPIO character device.
    path: PathBuf,
    _f: fs::File,
    /// Cached copy of _f.as_raw_fd() for syscalls, to avoid Arc<Mutex<>> overheads for ops.
    pub(crate) fd: RawFd,
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    abiv: RefCell<Option<AbiVersion>>,
}

impl Chip {
    /// Constructs a Chip using the given path.
    ///
    /// The path must resolve to a valid GPIO character device.
    pub fn from_path<P: AsRef<Path>>(p: P) -> Result<Chip> {
        let path = is_chip(p.as_ref())?;
        let f = fs::File::open(&path).map_err(crate::errno_from_ioerr)?;
        let fd = f.as_raw_fd();
        Ok(Chip {
            path,
            _f: f,
            fd,
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            abiv: Default::default(),
        })
    }

    /// Constructs a Chip using the given name.
    ///
    /// The name must resolve to a valid GPIO character device.
    pub fn from_name(n: &str) -> Result<Chip> {
        let path = is_chip(format!("/dev/{}", n))?;
        let f = fs::File::open(&path).map_err(crate::errno_from_ioerr)?;
        let fd = f.as_raw_fd();
        Ok(Chip {
            path,
            _f: f,
            fd,
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            abiv: Default::default(),
        })
    }

    /// Get the information for the chip.
    pub fn info(&self) -> Result<Info> {
        Ok(Info::from(
            uapi::get_chip_info(self.fd).map_err(|e| Error::UapiError(UapiCall::GetChipInfo, e))?,
        ))
    }

    /// Return the name of the chip.
    ///
    /// This is based on the filename component of the resolved chip path, not the name
    /// from the [`Info`], so it does not involve any system calls.
    ///
    /// [`Info`]: Info
    pub fn name(&self) -> String {
        // The unwrap can only fail for directories, and the path is known to refer to a file.
        String::from(self.path.file_name().unwrap().to_string_lossy())
    }

    /// Return the path of the chip.
    pub fn path(&self) -> &Path {
        self.path.as_ref()
    }

    // determine the actual abi version to use for subsequent uAPI operations.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn actual_abi_version(&self) -> Result<AbiVersion> {
        let mut abiv = self.abiv.borrow_mut();
        if abiv.is_none() {
            *abiv = Some(self.detect_abi_version()?);
        }
        Ok(abiv.unwrap())
    }

    /// Find the info for the named line.
    ///
    /// Returns the first matching line.
    pub fn find_line_info(&self, name: &str) -> Option<line::Info> {
        if let Ok(iter) = self.line_info_iter() {
            return iter.filter_map(|x| x.ok()).find(|li| li.name == name);
        }
        None
    }

    /// Get the information for a line on the chip.
    pub fn line_info(&self, offset: Offset) -> Result<line::Info> {
        self.do_line_info(offset)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_line_info(&self, offset: Offset) -> Result<line::Info> {
        let res = match self.actual_abi_version()? {
            V1 => v1::get_line_info(self.fd, offset).map(|li| line::Info::from(&li)),
            V2 => v2::get_line_info(self.fd, offset).map(|li| line::Info::from(&li)),
        };
        res.map_err(|e| Error::UapiError(UapiCall::GetLineInfo, e))
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_line_info(&self, offset: Offset) -> Result<line::Info> {
        uapi::get_line_info(self.fd, offset)
            .map(|li| line::Info::from(&li))
            .map_err(|e| Error::UapiError(UapiCall::GetLineInfo, e))
    }

    /// An iterator that returns the info for each line on the chip.
    pub fn line_info_iter(&self) -> Result<LineInfoIterator> {
        let cinfo = self.info()?;
        Ok(LineInfoIterator {
            chip: self,
            offsets: Range {
                start: 0,
                end: cinfo.num_lines,
            },
        })
    }

    /// Add a watch for changes to the publicly available information on a line.
    ///
    /// This is a null operation if there is already a watch on the line.
    pub fn watch_line_info(&self, offset: Offset) -> Result<line::Info> {
        self.do_watch_line_info(offset)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_watch_line_info(&self, offset: Offset) -> Result<line::Info> {
        let res = match self.actual_abi_version()? {
            V1 => v1::watch_line_info(self.fd, offset).map(|li| line::Info::from(&li)),
            V2 => v2::watch_line_info(self.fd, offset).map(|li| line::Info::from(&li)),
        };
        res.map_err(|e| Error::UapiError(UapiCall::WatchLineInfo, e))
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_watch_line_info(&self, offset: Offset) -> Result<line::Info> {
        uapi::watch_line_info(self.fd, offset)
            .map(|li| line::Info::from(&li))
            .map_err(|e| Error::UapiError(UapiCall::WatchLineInfo, e))
    }

    /// Remove a watch for changes to the publicly available information on a line.
    ///
    /// This is a null operation if there is no existing watch on the line.
    pub fn unwatch_line_info(&self, offset: Offset) -> Result<()> {
        uapi::unwatch_line_info(self.fd, offset)
            .map_err(|e| Error::UapiError(UapiCall::UnwatchLineInfo, e))
    }

    /// Check if the request has at least one info change event available to read.
    pub fn has_line_info_change_event(&self) -> Result<bool> {
        gpiocdev_uapi::has_event(self.fd).map_err(|e| Error::UapiError(UapiCall::HasEvent, e))
    }

    /// Wait for an info change event to be available.
    pub fn wait_line_info_change_event(&self, timeout: Duration) -> Result<bool> {
        gpiocdev_uapi::wait_event(self.fd, timeout)
            .map_err(|e| Error::UapiError(UapiCall::WaitEvent, e))
    }

    /// Read a single line info change event from the chip.
    ///
    /// Will block until an edge event is available.
    pub fn read_line_info_change_event(&self) -> Result<InfoChangeEvent> {
        self.do_read_line_info_change_event()
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_read_line_info_change_event(&self) -> Result<InfoChangeEvent> {
        self.actual_abi_version()?;
        // bbuf is statically sized to the greater of the v1/v2 size so it can be placed on the stack.
        debug_assert!(
            mem::size_of::<v2::LineInfoChangeEvent>() >= mem::size_of::<v1::LineInfoChangeEvent>()
        );
        let mut bbuf = [0; mem::size_of::<v2::LineInfoChangeEvent>()];
        let evsize = self.line_info_change_event_size();
        // and dynamically sliced down to the required size, if necessary
        let buf = &mut bbuf[0..evsize];
        let n = gpiocdev_uapi::read_event(self.fd, buf)
            .map_err(|e| Error::UapiError(UapiCall::ReadEvent, e))?;
        self.line_info_change_event_from_slice(&buf[0..n])
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_read_line_info_change_event(&self) -> Result<InfoChangeEvent> {
        let mut buf = [0; mem::size_of::<uapi::LineInfoChangeEvent>()];
        let n = gpiocdev_uapi::read_event(self.fd, &mut buf)
            .map_err(|e| Error::UapiError(UapiCall::ReadEvent, e))?;
        self.line_info_change_event_from_slice(&buf[0..n])
    }

    /// An iterator for info change events from the chip.
    pub fn info_change_events(&self) -> InfoChangeIterator {
        InfoChangeIterator {
            chip: self,
            buf: vec![0; self.line_info_change_event_size()],
        }
    }

    /// Detect the most recent uAPI ABI supported by the library for the chip.
    pub fn detect_abi_version(&self) -> Result<AbiVersion> {
        // check in preferred order
        for abiv in [V2, V1] {
            if self.supports_abi_version(abiv).is_ok() {
                return Ok(abiv);
            }
        }
        Err(Error::NoAbiSupport())
    }

    /// Check if the platform and library support a specific ABI version.
    pub fn supports_abi_version(&self, abiv: AbiVersion) -> Result<()> {
        self.do_supports_abi_version(abiv)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_supports_abi_version(&self, abiv: AbiVersion) -> Result<()> {
        let res = match abiv {
            V1 => v1::get_line_info(self.fd, 0).map(|_| ()),
            V2 => v2::get_line_info(self.fd, 0).map(|_| ()),
        };
        res.map_err(|_| Error::UnsupportedAbi(abiv, AbiSupportKind::Platform))
    }
    #[cfg(all(feature = "uapi_v1", not(feature = "uapi_v2")))]
    fn do_supports_abi_version(&self, abiv: AbiVersion) -> Result<()> {
        match abiv {
            V1 => uapi::get_line_info(self.fd, 0)
                .map(|_| ())
                .map_err(|_| Error::UnsupportedAbi(V1, AbiSupportKind::Platform)),
            V2 => Err(Error::UnsupportedAbi(
                AbiVersion::V2,
                AbiSupportKind::Library,
            )),
        }
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_supports_abi_version(&self, abiv: AbiVersion) -> Result<()> {
        match abiv {
            V2 => uapi::get_line_info(self.fd, 0)
                .map(|_| ())
                .map_err(|_| Error::UnsupportedAbi(V2, AbiSupportKind::Platform)),
            V1 => Err(Error::UnsupportedAbi(
                AbiVersion::V1,
                AbiSupportKind::Library,
            )),
        }
    }

    /// Set the ABI version to use for subsequent operations.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    pub fn using_abi_version(&mut self, abiv: AbiVersion) -> &mut Self {
        self.abiv = RefCell::new(Some(abiv));
        self
    }

    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn line_info_change_event_from_slice(&self, d: &[u8]) -> Result<InfoChangeEvent> {
        Ok(match self.actual_abi_version()? {
            V1 => InfoChangeEvent::from(
                v1::LineInfoChangeEvent::from_slice(d)
                    .map_err(|e| Error::UapiError(UapiCall::LICEFromBuf, e))?,
            ),
            V2 => InfoChangeEvent::from(
                v2::LineInfoChangeEvent::from_slice(d)
                    .map_err(|e| Error::UapiError(UapiCall::LICEFromBuf, e))?,
            ),
        })
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn line_info_change_event_from_slice(&self, d: &[u8]) -> Result<InfoChangeEvent> {
        Ok(InfoChangeEvent::from(
            uapi::LineInfoChangeEvent::from_slice(d)
                .map_err(|e| Error::UapiError(UapiCall::LICEFromBuf, e))?,
        ))
    }

    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn line_info_change_event_size(&self) -> usize {
        // may not be initialised if called via info_change_events()
        // in which case return the larger of the two
        match self.abiv.borrow().unwrap_or(V2) {
            V1 => mem::size_of::<v1::LineInfoChangeEvent>(),
            V2 => mem::size_of::<v2::LineInfoChangeEvent>(),
        }
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn line_info_change_event_size(&self) -> usize {
        mem::size_of::<uapi::LineInfoChangeEvent>()
    }
}
impl AsRawFd for Chip {
    fn as_raw_fd(&self) -> i32 {
        self.fd
    }
}

/// The publicly available information for a GPIO chip.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Info {
    /// The system name for the chip, such as "*gpiochip0*".
    pub name: String,

    /// A functional name for the chip.
    ///
    /// This typically identifies the type of GPIO chip.
    pub label: String,

    /// The number of lines provided by the chip.
    pub num_lines: u32,
}
impl From<uapi::ChipInfo> for Info {
    fn from(ci: uapi::ChipInfo) -> Self {
        Info {
            name: String::from(&ci.name),
            label: String::from(&ci.label),
            num_lines: ci.num_lines,
        }
    }
}

/// An iterator for reading info change events from a [`Chip`].
///
/// Blocks until events are available.
pub struct InfoChangeIterator<'a> {
    chip: &'a Chip,

    /// The buffer for uAPI edge events, sized by event size and capacity
    buf: Vec<u8>,
}

impl<'a> InfoChangeIterator<'a> {
    fn read_event(&mut self) -> Result<InfoChangeEvent> {
        let n = gpiocdev_uapi::read_event(self.chip.fd, &mut self.buf)
            .map_err(|e| Error::UapiError(UapiCall::ReadEvent, e))?;
        self.chip.line_info_change_event_from_slice(&self.buf[0..n])
    }
}

impl<'a> Iterator for InfoChangeIterator<'a> {
    type Item = Result<InfoChangeEvent>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.read_event())
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
    use gpiocdev_uapi::v1 as uapi;
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    use gpiocdev_uapi::v2 as uapi;

    // Chip, ChipIterator and InfoChangeIterator tests are all integration
    // tests as Chip construction requires GPIO chips.

    mod info {
        use super::{uapi, Info};

        #[test]
        fn from_uapi() {
            let ui = uapi::ChipInfo {
                name: "banana".into(),
                label: "peel".into(),
                num_lines: 42,
            };
            let i = Info::from(ui);
            assert_eq!(i.num_lines, 42);
            assert_eq!(i.name.as_str(), "banana");
            assert_eq!(i.label.as_str(), "peel");
        }
    }
}
