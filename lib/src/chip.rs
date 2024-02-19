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
#[cfg(feature = "serde")]
use serde_derive::{Deserialize, Serialize};
#[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
use std::cell::Cell;
use std::fmt;
use std::fs;
use std::mem;
use std::ops::Range;
use std::os::linux::fs::MetadataExt;
use std::os::unix::prelude::{AsFd, AsRawFd, BorrowedFd, OsStrExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

const CHARDEV_MODE: u32 = 0x2000;

/// Check if a path corresponds to a GPIO character device.
///
/// Returns the resolved path to the character device.
pub fn is_chip<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    let pb = fs::canonicalize(&path)?;
    // if canonical path is of form /dev/gpiochipXX assume we are good
    if let Some(pbstr) = pb.to_str() {
        if let Some(num) = pbstr.strip_prefix("/dev/gpiochip") {
            if num.chars().all(|c| char::is_digit(c, 10)) {
                return Ok(pb);
            }
        }
    }

    // else take a more detailed look...
    let m = fs::metadata(&pb)?;
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

/// Compare two chip paths.
///
// Sorts paths naturally, assuming any chip numbering is at the end of the path - as it is for gpiochips.
pub fn path_compare(a: &Path, b: &Path) -> std::cmp::Ordering {
    let a = a.as_os_str().as_bytes();
    let b = b.as_os_str().as_bytes();

    if a.len() == b.len() {
        // if equal length then just compare lexicographically
        return a.cmp(b);
    }
    for it in a.iter().zip(b.iter()) {
        let (ai, bi) = it;
        if *ai != *bi {
            if !ai.is_ascii_digit() || !bi.is_ascii_digit() {
                // if either is not a digit then this character is definitive
                return (*ai).cmp(bi);
            }
            // else drop thru to length comparison
            break;
        }
    }
    // equal up to to the length of the shortest - or to digits and shorter numbers are smaller
    a.len().cmp(&b.len())
}

/// Returns the paths of all the GPIO character devices on the system.
///
/// The returned paths are sorted in name order and are confirmed to be GPIO character devices,
/// so there is no need to check them with [`is_chip`].
pub fn chips() -> Result<Vec<PathBuf>> {
    let mut chips = std::fs::read_dir("/dev")?
        .filter_map(|x| x.ok())
        .flat_map(|de| is_chip(de.path()))
        .collect::<Vec<PathBuf>>();
    chips.sort_unstable_by(|a, b| path_compare(a, b));
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
        self.offsets
            .next()
            .map(|offset| self.chip.line_info(offset))
    }
}

/// A GPIO character device.
#[derive(Debug)]
pub struct Chip {
    /// The resolved path of the GPIO character device.
    path: PathBuf,
    /// The open GPIO character device file.
    pub(crate) f: fs::File,
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    abiv: Cell<Option<AbiVersion>>,
}

impl Chip {
    /// Constructs a Chip using the given path.
    ///
    /// The path must resolve to a valid GPIO character device.
    ///
    /// # Examples
    ///```no_run
    /// # fn example() -> gpiocdev::Result<gpiocdev::Chip>{
    /// let chip = gpiocdev::Chip::from_path("/dev/gpiochip0")?;
    /// # Ok(chip)
    /// # }
    ///```
    pub fn from_path<P: AsRef<Path>>(p: P) -> Result<Chip> {
        let path = is_chip(p.as_ref())?;
        let f = fs::File::open(&path)?;
        Ok(Chip {
            path,
            f,
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            abiv: Default::default(),
        })
    }

    /// Constructs a Chip using the given name.
    ///
    /// The name must resolve to a valid GPIO character device.
    ///
    /// # Examples
    ///```no_run
    /// # fn example() -> gpiocdev::Result<gpiocdev::Chip>{
    /// let chip = gpiocdev::Chip::from_name("gpiochip0")?;
    /// # Ok(chip)
    /// # }
    ///```
    pub fn from_name(n: &str) -> Result<Chip> {
        let path = is_chip(format!("/dev/{}", n))?;
        let f = fs::File::open(&path)?;
        Ok(Chip {
            path,
            f,
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            abiv: Default::default(),
        })
    }

    /// Get the information for the chip.
    pub fn info(&self) -> Result<Info> {
        Ok(Info::from(
            uapi::get_chip_info(&self.f).map_err(|e| Error::Uapi(UapiCall::GetChipInfo, e))?,
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
        Ok(match self.abiv.get() {
            Some(abiv) => abiv,
            None => {
                let abiv = self.detect_abi_version()?;
                self.abiv.set(Some(abiv));
                abiv
            }
        })
    }

    /// Find the info for the named line.
    ///
    /// Returns the first matching line.
    pub fn find_line_info(&self, name: &str) -> Option<line::Info> {
        self.line_info_iter()
            .ok()
            .and_then(|iter| iter.filter_map(|x| x.ok()).find(|li| li.name == name))
    }

    /// Get the information for a line on the chip.
    pub fn line_info(&self, offset: Offset) -> Result<line::Info> {
        self.do_line_info(offset)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_line_info(&self, offset: Offset) -> Result<line::Info> {
        match self.actual_abi_version()? {
            V1 => v1::get_line_info(&self.f, offset).map(|li| line::Info::from(&li)),
            V2 => v2::get_line_info(&self.f, offset).map(|li| line::Info::from(&li)),
        }
        .map_err(|e| Error::Uapi(UapiCall::GetLineInfo, e))
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_line_info(&self, offset: Offset) -> Result<line::Info> {
        uapi::get_line_info(&self.f, offset)
            .map(|li| line::Info::from(&li))
            .map_err(|e| Error::Uapi(UapiCall::GetLineInfo, e))
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
        match self.actual_abi_version()? {
            V1 => v1::watch_line_info(&self.f, offset).map(|li| line::Info::from(&li)),
            V2 => v2::watch_line_info(&self.f, offset).map(|li| line::Info::from(&li)),
        }
        .map_err(|e| Error::Uapi(UapiCall::WatchLineInfo, e))
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_watch_line_info(&self, offset: Offset) -> Result<line::Info> {
        uapi::watch_line_info(&self.f, offset)
            .map(|li| line::Info::from(&li))
            .map_err(|e| Error::Uapi(UapiCall::WatchLineInfo, e))
    }

    /// Remove a watch for changes to the publicly available information on a line.
    ///
    /// This is a null operation if there is no existing watch on the line.
    pub fn unwatch_line_info(&self, offset: Offset) -> Result<()> {
        uapi::unwatch_line_info(&self.f, offset)
            .map_err(|e| Error::Uapi(UapiCall::UnwatchLineInfo, e))
    }

    /// Check if the request has at least one info change event available to read.
    pub fn has_line_info_change_event(&self) -> Result<bool> {
        gpiocdev_uapi::has_event(&self.f).map_err(|e| Error::Uapi(UapiCall::HasEvent, e))
    }

    /// Wait for an info change event to be available.
    pub fn wait_line_info_change_event(&self, timeout: Duration) -> Result<bool> {
        gpiocdev_uapi::wait_event(&self.f, timeout).map_err(|e| Error::Uapi(UapiCall::WaitEvent, e))
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
        let mut bbuf = [0_u64; mem::size_of::<v2::LineInfoChangeEvent>() / 8];
        let evt_u64_size = self.line_info_change_event_u64_size();
        // and dynamically sliced down to the required size, if necessary
        let buf = &mut bbuf[0..evt_u64_size];
        let n = gpiocdev_uapi::read_event(&self.f, buf)
            .map_err(|e| Error::Uapi(UapiCall::ReadEvent, e))?;
        self.line_info_change_event_from_slice(&buf[0..n])
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_read_line_info_change_event(&self) -> Result<InfoChangeEvent> {
        let mut buf = [0_u64; mem::size_of::<uapi::LineInfoChangeEvent>() / 8];
        let n = gpiocdev_uapi::read_event(&self.f, &mut buf)
            .map_err(|e| Error::Uapi(UapiCall::ReadEvent, e))?;
        self.line_info_change_event_from_slice(&buf[0..n])
    }

    /// An iterator for info change events from the chip.
    pub fn info_change_events(&self) -> InfoChangeIterator {
        InfoChangeIterator {
            chip: self,
            buf: vec![0_u64; self.line_info_change_event_u64_size()],
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
            V1 => v1::get_line_info(&self.f, 0).map(|_| ()),
            V2 => v2::get_line_info(&self.f, 0).map(|_| ()),
        };
        res.map_err(|_| Error::UnsupportedAbi(abiv, AbiSupportKind::Kernel))
    }
    #[cfg(all(feature = "uapi_v1", not(feature = "uapi_v2")))]
    fn do_supports_abi_version(&self, abiv: AbiVersion) -> Result<()> {
        match abiv {
            V1 => uapi::get_line_info(&self.f, 0)
                .map(|_| ())
                .map_err(|_| Error::UnsupportedAbi(V1, AbiSupportKind::Kernel)),
            V2 => Err(Error::UnsupportedAbi(AbiVersion::V2, AbiSupportKind::Build)),
        }
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_supports_abi_version(&self, abiv: AbiVersion) -> Result<()> {
        match abiv {
            V2 => uapi::get_line_info(&self.f, 0)
                .map(|_| ())
                .map_err(|_| Error::UnsupportedAbi(V2, AbiSupportKind::Kernel)),
            V1 => Err(Error::UnsupportedAbi(AbiVersion::V1, AbiSupportKind::Build)),
        }
    }

    /// Set the ABI version to use for subsequent operations.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    pub fn using_abi_version(&mut self, abiv: AbiVersion) -> &mut Self {
        self.abiv.set(Some(abiv));
        self
    }

    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn line_info_change_event_from_slice(&self, d: &[u64]) -> Result<InfoChangeEvent> {
        Ok(match self.actual_abi_version()? {
            V1 => InfoChangeEvent::from(
                v1::LineInfoChangeEvent::from_slice(d)
                    .map_err(|e| Error::Uapi(UapiCall::LICEFromBuf, e))?,
            ),
            V2 => InfoChangeEvent::from(
                v2::LineInfoChangeEvent::from_slice(d)
                    .map_err(|e| Error::Uapi(UapiCall::LICEFromBuf, e))?,
            ),
        })
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn line_info_change_event_from_slice(&self, d: &[u64]) -> Result<InfoChangeEvent> {
        Ok(InfoChangeEvent::from(
            uapi::LineInfoChangeEvent::from_slice(d)
                .map_err(|e| Error::Uapi(UapiCall::LICEFromBuf, e))?,
        ))
    }

    fn line_info_change_event_u64_size(&self) -> usize {
        self.line_info_change_event_size() / 8
    }

    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn line_info_change_event_size(&self) -> usize {
        // may not be initialised if called via info_change_events()
        // in which case return the larger of the two
        match self.abiv.get().unwrap_or(V2) {
            V1 => mem::size_of::<v1::LineInfoChangeEvent>(),
            V2 => mem::size_of::<v2::LineInfoChangeEvent>(),
        }
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn line_info_change_event_size(&self) -> usize {
        mem::size_of::<uapi::LineInfoChangeEvent>()
    }
}

impl AsFd for Chip {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.f.as_fd()
    }
}

impl AsRawFd for Chip {
    #[inline]
    fn as_raw_fd(&self) -> i32 {
        self.f.as_raw_fd()
    }
}

impl AsRef<Chip> for Chip {
    #[inline]
    fn as_ref(&self) -> &Chip {
        self
    }
}

/// The publicly available information for a GPIO chip.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "camelCase")
)]
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
    buf: Vec<u64>,
}

impl<'a> InfoChangeIterator<'a> {
    fn read_event(&mut self) -> Result<InfoChangeEvent> {
        let n = gpiocdev_uapi::read_event(&self.chip.f, &mut self.buf)
            .map_err(|e| Error::Uapi(UapiCall::ReadEvent, e))?;
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

    #[test]
    fn path_compare() {
        use super::path_compare;
        use std::cmp::Ordering;

        assert_eq!(
            path_compare(Path::new("/dev/gpiochip0"), Path::new("/dev/gpiochip0")),
            Ordering::Equal
        );
        assert_eq!(
            path_compare(Path::new("/dev/gpiochip0"), Path::new("/dev/gpiochip1")),
            Ordering::Less
        );
        assert_eq!(
            path_compare(Path::new("/dev/gpiochip3"), Path::new("/dev/gpiochip10")),
            Ordering::Less
        );
        assert_eq!(
            path_compare(Path::new("/dev/gpiochip3"), Path::new("/dev/gpiochip30")),
            Ordering::Less
        );
        assert_eq!(
            path_compare(Path::new("/dev/gpiochip10"), Path::new("/dev/gpiochip3")),
            Ordering::Greater
        );
        assert_eq!(
            path_compare(Path::new("/dev/gpiochip"), Path::new("/dev/gpiochip1")),
            Ordering::Less
        );
        assert_eq!(
            path_compare(Path::new("/dev/gpiechip0"), Path::new("/dev/gpiochip1")),
            Ordering::Less
        );
    }
}
