// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use anyhow::{Context, Result};
use clap::Parser;
use gpiocdev::chip::{chips, Chip};
use gpiocdev::line::Offset;
use gpiocdev::request::Config;
use gpiocdev::{
    detect_abi_version,
    AbiVersion::{self, *},
};
use std::collections::HashMap;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use strum::{EnumString, EnumVariantNames, VariantNames};

// common helper functions

pub fn all_chips() -> Result<Vec<PathBuf>> {
    let mut cc: Vec<PathBuf> = chips().context("Failed to find any chips.")?.collect();
    // sorted for consistent outputs
    cc.sort();
    Ok(cc)
}

pub fn chip_from_opts(p: &Path, abiv: u8) -> Result<Chip> {
    let mut c = Chip::from_path(p).with_context(|| format!("Failed to open chip {:?}.", &p))?;
    let abiv = match abiv {
        1 => V1,
        2 => V2,
        _ => c
            .detect_abi_version()
            .with_context(|| format!("Failed to detect ABI version on chip {:?}.", &p))?,
    };
    c.using_abi_version(abiv);
    Ok(c)
}

pub fn abi_version_from_opts(abiv: u8) -> Result<AbiVersion> {
    let abiv = match abiv {
        1 => V1,
        2 => V2,
        _ => detect_abi_version().context("Failed to detect ABI version.")?,
    };
    Ok(abiv)
}

pub fn parse_chip_path(s: &OsStr) -> PathBuf {
    let str = s.to_string_lossy();
    if str.chars().all(char::is_numeric) {
        // from number
        let mut p = OsString::from("/dev/gpiochip");
        p.push(s);
        return PathBuf::from(p);
    }
    if !str.chars().any(|x| x == '/') {
        // from name
        let mut p = OsString::from("/dev/");
        p.push(s);
        return PathBuf::from(p);
    }
    // from raw path
    PathBuf::from(s)
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct ChipOffset {
    pub chip: PathBuf,
    pub offset: Offset,
}

pub fn find_lines(
    lines: &[String],
    opts: &LineOpts,
    abiv: u8,
) -> Result<(HashMap<String, ChipOffset>, Vec<PathBuf>)> {
    let chips = match &opts.chip {
        Some(chip_path) => vec![chip_path.clone()],
        None => all_chips()?,
    };

    let mut found_lines = HashMap::new();
    let mut fchips = Vec::new();
    if chips.len() == 1 && !opts.by_name {
        let chip = chips.get(0).unwrap();
        for line_id in lines {
            if let Ok(offset) = line_id.parse::<u32>() {
                found_lines.insert(
                    line_id.to_owned(),
                    ChipOffset {
                        chip: chip.clone(),
                        offset,
                    },
                );
            }
        }
    }

    for path in &chips {
        let chip = chip_from_opts(path, abiv)?;
        let ci = chip
            .info()
            .with_context(|| format!("Failed to info from chip {:?}.", path))?;
        for offset in 0..ci.num_lines {
            let li = chip.line_info(offset).with_context(|| {
                format!("Failed to read line {} info from chip {:?}.", offset, path)
            })?;
            for name in lines {
                if name.as_str() == li.name.as_os_str() {
                    if !found_lines.contains_key(name) {
                        found_lines.insert(
                            name.to_owned(),
                            ChipOffset {
                                chip: path.to_owned(),
                                offset,
                            },
                        );
                        if !opts.strict && found_lines.len() == lines.len() {
                            break;
                        }
                    } else if opts.strict {
                        return Err(anyhow::Error::new(DuplicateLineError::new(name)));
                    }
                }
            }
        }
    }
    for line_id in lines {
        let co = found_lines
            .get(line_id)
            .context(format!("Can't find line {:?}.", line_id))?;
        if !fchips.contains(&co.chip) {
            fchips.push(co.chip.clone());
        }
    }

    Ok((found_lines, fchips))
}

#[derive(Debug, thiserror::Error)]
pub enum ParseDurationError {
    #[error("'{0}' missing units - add \"s\", \"ms\", \"us\" or \"ns\".")]
    NoUnits(String),
    #[error("'{0}' unknown units - use \"s\", \"ms\", \"us\" or \"ns\".")]
    Units(String),
    #[error(transparent)]
    Digits(std::num::ParseIntError),
}

pub fn parse_duration(s: &str) -> std::result::Result<Duration, ParseDurationError> {
    let t = match s.find(|c: char| !c.is_ascii_digit()) {
        Some(n) => {
            let (num, units) = s.split_at(n);
            let t = num.parse::<u64>().map_err(ParseDurationError::Digits)?;
            t * match units {
                "ns" => 1,
                "us" => 1000,
                "ms" => 1000000,
                "s" => 1000000000,
                _ => return Err(ParseDurationError::Units(s.to_string())),
            }
        }
        None => return Err(ParseDurationError::NoUnits(s.to_string())),
    };
    Ok(Duration::from_nanos(t))
}

pub fn string_or_default<'a, 'b: 'a>(s: &'a str, def: &'b str) -> &'a str {
    if s.is_empty() {
        def
    } else {
        s
    }
}

// common command line parser options

#[derive(Debug, Parser)]
/// Options to control the selection of lines.
pub struct LineOpts {
    /// Restrict scope to the lines on this chip.
    ///
    /// If specified then lines may be identified by either name or offset.
    ///
    /// If not specified then the named lines are searched for on all chips in the system.
    ///
    /// The chip may be identified by number, name, or path.
    /// e.g. the following all select the same chip:
    ///     --chip 0
    ///     --chip gpiochip0
    ///     --chip /dev/gpiochip0
    #[clap(short, long, name="chip", parse(from_os_str = parse_chip_path), verbatim_doc_comment)]
    pub chip: Option<PathBuf>,
    /// Provided line names must be unique or the command will abort.
    ///
    /// If --chip is also specified then provided line names must only be unique to the
    /// lines on that chip.
    #[clap(short = 's', long)]
    pub strict: bool,
    /// Lines are strictly identified by name.
    ///
    /// If --chip is provided then lines are initially assumed to be offsets, and only
    /// fallback to names if the line does not parse as an offset.
    ///
    /// With --by-name set the lines are always identified by name, never as offsets.
    #[clap(short = 'N', long)]
    pub by_name: bool,
}

#[derive(Debug, Parser)]
pub struct UapiOpts {
    /// The uAPI ABI version to use to perform the operation.
    ///
    /// The default option detects the uAPI versions supported by the kernel and uses the latest.
    // This is primarily aimed at debugging and so is a hidden option.
    #[clap(long, default_value = "0", hide = true, env = "GPIOD_ABI_VERSION")]
    pub abiv: u8,
}

#[derive(Debug, Parser)]
pub struct ActiveLowOpts {
    /// Treat the line as active-low when determining value.
    #[clap(short = 'l', long)]
    pub active_low: bool,
}
impl ActiveLowOpts {
    pub fn apply<'b>(&self, r: &'b mut Config) -> &'b mut Config {
        if self.active_low {
            return r.as_active_low();
        }
        r
    }
}

#[derive(EnumString, EnumVariantNames, Copy, Clone, Debug)]
#[strum(serialize_all = "kebab_case")]
pub enum BiasFlags {
    PullUp,
    PullDown,
    Disabled,
}
impl From<BiasFlags> for gpiocdev::line::Bias {
    fn from(b: BiasFlags) -> Self {
        match b {
            BiasFlags::PullUp => gpiocdev::line::Bias::PullUp,
            BiasFlags::PullDown => gpiocdev::line::Bias::PullDown,
            BiasFlags::Disabled => gpiocdev::line::Bias::Disabled,
        }
    }
}

#[derive(Copy, Clone, Debug, Parser)]
pub struct BiasOpts {
    /// The bias to be applied to the lines.
    #[clap(short, long, name = "bias", possible_values = BiasFlags::VARIANTS, ignore_case = true)]
    pub bias: Option<BiasFlags>,
}
impl BiasOpts {
    pub fn apply(self, r: &mut Config) -> &mut Config {
        if let Some(bias) = self.bias {
            r.with_bias(Some(bias.into()));
        }
        r
    }
}

#[derive(EnumString, EnumVariantNames, Copy, Clone, Debug)]
#[strum(serialize_all = "kebab_case")]
pub enum DriveFlags {
    PushPull,
    OpenDrain,
    OpenSource,
}
impl From<DriveFlags> for gpiocdev::line::Drive {
    fn from(b: DriveFlags) -> Self {
        match b {
            DriveFlags::PushPull => gpiocdev::line::Drive::PushPull,
            DriveFlags::OpenDrain => gpiocdev::line::Drive::OpenDrain,
            DriveFlags::OpenSource => gpiocdev::line::Drive::OpenSource,
        }
    }
}
#[derive(Copy, Clone, Debug, Parser)]
pub struct DriveOpts {
    /// How the lines should be driven.
    #[clap(short, long, name = "drive", possible_values = DriveFlags::VARIANTS, ignore_case = true)]
    pub drive: Option<DriveFlags>,
}
impl DriveOpts {
    pub fn apply(self, r: &mut Config) -> &mut Config {
        if let Some(drive) = self.drive {
            r.with_drive(drive.into());
        }
        r
    }
}

#[derive(EnumString, EnumVariantNames, Copy, Clone, Debug)]
#[strum(serialize_all = "kebab_case")]
pub enum EdgeFlags {
    RisingEdge,
    FallingEdge,
    BothEdges,
}
impl From<EdgeFlags> for gpiocdev::line::EdgeDetection {
    fn from(b: EdgeFlags) -> Self {
        match b {
            EdgeFlags::RisingEdge => gpiocdev::line::EdgeDetection::RisingEdge,
            EdgeFlags::FallingEdge => gpiocdev::line::EdgeDetection::FallingEdge,
            EdgeFlags::BothEdges => gpiocdev::line::EdgeDetection::BothEdges,
        }
    }
}
#[derive(Copy, Clone, Debug, Parser)]
pub struct EdgeOpts {
    /// Which edges should be detected and reported.
    #[clap(short, long, name = "edge", possible_values = EdgeFlags::VARIANTS, default_value="both-edges", ignore_case = true)]
    pub edge: EdgeFlags,
}
impl EdgeOpts {
    pub fn apply(self, r: &mut Config) -> &mut Config {
        r.with_edge_detection(Some(self.edge.into()))
    }
}

#[derive(Clone, Debug, Parser)]
pub struct LinesOpts {}

pub fn stringify_attrs(li: &gpiocdev::line::Info) -> String {
    use gpiocdev::line::{Bias, Direction, Drive, EdgeDetection, EventClock};

    let mut attrs = Vec::new();
    match li.direction {
        gpiocdev::line::Direction::Input => attrs.push("input"),
        Direction::Output => attrs.push("output"),
    }
    if li.used {
        attrs.push("used");
    }
    if li.active_low {
        attrs.push("active-low");
    }
    match li.drive {
        None => (),
        Some(Drive::PushPull) => (),
        Some(Drive::OpenDrain) => attrs.push("open-drain"),
        Some(Drive::OpenSource) => attrs.push("open-source"),
    }
    match li.bias {
        None => (),
        Some(Bias::PullUp) => attrs.push("pull-up"),
        Some(Bias::PullDown) => attrs.push("pull-down"),
        Some(Bias::Disabled) => attrs.push("bias-disabled"),
    }
    match li.edge_detection {
        None => (),
        Some(EdgeDetection::RisingEdge) => attrs.push("rising-edge"),
        Some(EdgeDetection::FallingEdge) => attrs.push("falling-edge"),
        Some(EdgeDetection::BothEdges) => attrs.push("both-edges"),
    }
    match li.event_clock {
        None => (),                        // Not present for v1.
        Some(EventClock::Monotonic) => (), // default for ABI v2
        Some(EventClock::Realtime) => attrs.push("event-clock-realtime"),
    }
    let db;
    if li.debounce_period.is_some() {
        db = format!("debounce_period={:?}", li.debounce_period.unwrap());
        attrs.push(&db);
    }
    attrs.join(", ")
}

#[derive(Debug)]
struct DuplicateLineError {
    name: String,
}

impl DuplicateLineError {
    pub fn new<S: Into<String>>(name: S) -> DuplicateLineError {
        DuplicateLineError { name: name.into() }
    }
}

impl fmt::Display for DuplicateLineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "duplicate line name: `{}`.", self.name)
    }
}
impl Error for DuplicateLineError {}
