// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use gpiocdev::chip::{self, Chip};
use gpiocdev::line::{Bias, Drive, EdgeDetection, Info, Offset};
use gpiocdev::request::Config;
use gpiocdev::{self, AbiVersion};
use nohash_hasher::IntMap;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

// common helper functions

pub fn all_chip_paths() -> Result<Vec<PathBuf>> {
    let mut cc: Vec<PathBuf> = chip::chips().context("unable to find any chips")?.collect();
    // sorted for consistent outputs
    cc.sort();
    Ok(cc)
}

pub fn chip_from_path(p: &Path, abiv: u8) -> Result<Chip> {
    let mut c =
        Chip::from_path(p).with_context(|| format!("unable to open chip '{}'", p.display()))?;
    let abiv = match abiv {
        1 => AbiVersion::V1,
        2 => AbiVersion::V2,
        _ => c
            .detect_abi_version()
            .with_context(|| format!("unable to detect ABI version on {}", c.name()))?,
    };
    c.using_abi_version(abiv);
    Ok(c)
}

pub fn abi_version_from_opts(abiv: u8) -> Result<AbiVersion> {
    let abiv = match abiv {
        1 => AbiVersion::V1,
        2 => AbiVersion::V2,
        _ => gpiocdev::detect_abi_version().context("unable to detect ABI version")?,
    };
    Ok(abiv)
}

fn chip_path_from_id(id: &str) -> PathBuf {
    if id.chars().all(char::is_numeric) {
        // from number
        return format!("/dev/gpiochip{}", id).into();
    }
    if !id.chars().any(|x| x == '/') {
        // from name
        let mut p: PathBuf = "/dev".into();
        p.push(id);
        return p;
    }
    // from raw path
    id.into()
}

pub fn chip_lookup_from_id(id: &str) -> Result<PathBuf> {
    let path = chip_path_from_id(id);
    chip::is_chip(&path).with_context(|| format!("cannot find GPIO chip character device '{}'", id))
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct ChipOffset {
    // This is the idx into the Vec<ChipInfo>, not a system gpiochip#.
    pub chip_idx: usize,
    pub offset: Offset,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ChipInfo {
    pub path: PathBuf,
    pub name: String,
    pub named_lines: IntMap<Offset, String>,
}

pub struct Resolver {
    pub lines: HashMap<String, ChipOffset>,
    pub chips: Vec<ChipInfo>,
}

pub fn resolve_lines(lines: &[String], opts: &LineOpts, abiv: u8) -> Result<Resolver> {
    let chips = match &opts.chip {
        Some(chip_id) => vec![chip_path_from_id(chip_id)],
        None => all_chip_paths()?,
    };

    let mut r = Resolver {
        lines: HashMap::new(),
        chips: Vec::new(),
    };
    let mut chip_idx = 0;
    for (idx, path) in chips.iter().enumerate() {
        let found_count = r.lines.len();
        let chip = chip_from_path(path, abiv)?;
        let kci = chip
            .info()
            .with_context(|| format!("unable to read info from {}", chip.name()))?;
        let mut ci = ChipInfo {
            path: chip.path().to_owned(),
            name: kci.name,
            named_lines: IntMap::default(),
        };
        let mut used_lines = IntMap::default();

        // first match line by offset - but only when id by offset is possible
        if idx == 0 && opts.chip.is_some() && !opts.by_name {
            for id in lines {
                if let Ok(offset) = id.parse::<u32>() {
                    if offset < kci.num_lines {
                        if let Some(same_line) = used_lines.get(&offset) {
                            bail!(RepeatedLineError::new(same_line, id));
                        }
                        used_lines.insert(offset, id.to_owned());
                        r.lines
                            .insert(id.to_owned(), ChipOffset { chip_idx, offset });
                    }
                }
            }
        }
        // match by name
        for offset in 0..kci.num_lines {
            let li = chip.line_info(offset).with_context(|| {
                format!(
                    "unable to read info for line {} from {}",
                    offset,
                    chip.name()
                )
            })?;
            for id in lines {
                if id.as_str() == li.name.as_str() {
                    if let Some(same_line) = used_lines.get(&offset) {
                        bail!(RepeatedLineError::new(same_line, id));
                    }
                    if !r.lines.contains_key(id) {
                        used_lines.insert(offset, id.to_owned());
                        r.lines
                            .insert(id.to_owned(), ChipOffset { chip_idx, offset });
                        ci.named_lines.insert(offset, id.to_owned());

                        if !opts.strict && r.lines.len() == lines.len() {
                            break;
                        }
                    } else if opts.strict {
                        bail!(NonUniqueLineError::new(id));
                    }
                }
            }
        }
        if found_count != r.lines.len() {
            r.chips.push(ci);
            chip_idx += 1;
        }
    }
    for id in lines {
        if !r.lines.contains_key(id) {
            if !opts.by_name && id.parse::<u32>().is_ok() && opts.chip.is_some() {
                bail!(OffsetOutOfRangeError::new(id, opts.chip.as_ref().unwrap()));
            } else {
                bail!(LineNotFoundError::new(id));
            }
            // !!! collect, don't bail
        }
    }

    Ok(r)
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum ParseDurationError {
    #[error("'{0}' unknown units - use 's', 'ms' or 'us'.")]
    Units(String),
    #[error("'{0}' must start with a digit")]
    NoDigits(String),
}

pub fn parse_duration(s: &str) -> std::result::Result<Duration, ParseDurationError> {
    if s == "0" {
        return Ok(Duration::ZERO);
    }
    let t = match s.find(|c: char| !c.is_ascii_digit()) {
        Some(0) => return Err(ParseDurationError::NoDigits(s.to_string())),
        Some(n) => {
            let (num, units) = s.split_at(n);
            let t = num.parse::<u64>().unwrap();
            t * match units {
                "us" => 1000,
                "ms" => 1000000,
                "s" => 1000000000,
                _ => return Err(ParseDurationError::Units(s.to_string())),
            }
        }
        None => s.parse::<u64>().unwrap() * 1000000,
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
    /// Restrict scope to the lines on this chip
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
    #[arg(short, long, name = "chip", verbatim_doc_comment)]
    pub chip: Option<String>,

    /// Requested line names must be unique or the command will abort
    ///
    /// If --chip is also specified then requested line names must only be unique to the
    /// lines on that chip.
    #[arg(short = 's', long)]
    pub strict: bool,

    /// Lines are strictly identified by name
    ///
    /// If --chip is provided then lines are initially assumed to be offsets, and only
    /// fallback to names if the line does not parse as an offset.
    ///
    /// With --by-name set the lines are always identified by name, never as offsets.
    #[arg(long)]
    pub by_name: bool,
}

#[derive(Debug, Parser)]
pub struct UapiOpts {
    /// The uAPI ABI version to use to perform the operation
    ///
    /// The default option detects the uAPI versions supported by the kernel and uses the latest.
    // This is primarily aimed at debugging and so is a hidden option.
    #[arg(long, default_value = "0", hide = true, env = "GPIOD_ABI_VERSION")]
    pub abiv: u8,
}

#[derive(Debug, Parser)]
pub struct ActiveLowOpts {
    /// Treat the line as active-low when determining value
    #[arg(short = 'l', long)]
    pub active_low: bool,
}
impl ActiveLowOpts {
    pub fn apply(&self, r: &mut Config) {
        if self.active_low {
            r.as_active_low();
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum BiasFlags {
    PullUp,
    PullDown,
    Disabled,
}
impl From<BiasFlags> for Bias {
    fn from(b: BiasFlags) -> Self {
        match b {
            BiasFlags::PullUp => Bias::PullUp,
            BiasFlags::PullDown => Bias::PullDown,
            BiasFlags::Disabled => Bias::Disabled,
        }
    }
}

#[derive(Clone, Copy, Debug, Parser)]
pub struct BiasOpts {
    /// The bias to be applied to the lines
    ///
    /// By default the bias is left unchanged.
    #[arg(short, long, name = "bias", value_enum, ignore_case = true)]
    pub bias: Option<BiasFlags>,
}
impl BiasOpts {
    pub fn apply(self, r: &mut Config) {
        if let Some(bias) = self.bias {
            r.with_bias(Some(bias.into()));
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DriveFlags {
    PushPull,
    OpenDrain,
    OpenSource,
}
impl From<DriveFlags> for Drive {
    fn from(b: DriveFlags) -> Self {
        match b {
            DriveFlags::PushPull => Drive::PushPull,
            DriveFlags::OpenDrain => Drive::OpenDrain,
            DriveFlags::OpenSource => Drive::OpenSource,
        }
    }
}
#[derive(Clone, Copy, Debug, Parser)]
pub struct DriveOpts {
    /// How the lines should be driven
    #[arg(
        short,
        long,
        name = "drive",
        default_value = "push-pull",
        value_enum,
        ignore_case = true
    )]
    pub drive: Option<DriveFlags>,
}
impl DriveOpts {
    pub fn apply(self, r: &mut Config) {
        if let Some(drive) = self.drive {
            r.with_drive(drive.into());
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum EdgeFlags {
    Rising,
    Falling,
    Both,
}
impl From<EdgeFlags> for EdgeDetection {
    fn from(b: EdgeFlags) -> Self {
        match b {
            EdgeFlags::Rising => EdgeDetection::RisingEdge,
            EdgeFlags::Falling => EdgeDetection::FallingEdge,
            EdgeFlags::Both => EdgeDetection::BothEdges,
        }
    }
}
#[derive(Clone, Copy, Debug, Parser)]
pub struct EdgeOpts {
    /// Which edges should be detected and reported
    #[arg(
        short,
        long,
        name = "edges",
        value_enum,
        default_value = "both",
        ignore_case = true
    )]
    pub edges: EdgeFlags,
}
impl EdgeOpts {
    pub fn apply(self, r: &mut Config) {
        r.with_edge_detection(Some(self.edges.into()));
    }
}

#[derive(Clone, Debug, Parser)]
pub struct LinesOpts {}

pub fn stringify_attrs(li: &gpiocdev::line::Info) -> String {
    use gpiocdev::line::{Direction, EventClock};

    let mut attrs = Vec::new();
    match li.direction {
        gpiocdev::line::Direction::Input => attrs.push("input"),
        Direction::Output => attrs.push("output"),
    }
    if li.active_low {
        attrs.push("active-low");
    }
    match li.drive {
        None => (),
        Some(Drive::PushPull) => (),
        Some(Drive::OpenDrain) => attrs.push("drive=open-drain"),
        Some(Drive::OpenSource) => attrs.push("drive=open-source"),
    }
    match li.bias {
        None => (),
        Some(Bias::PullUp) => attrs.push("bias=pull-up"),
        Some(Bias::PullDown) => attrs.push("bias=pull-down"),
        Some(Bias::Disabled) => attrs.push("bias=disabled"),
    }
    match li.edge_detection {
        None => (),
        Some(EdgeDetection::RisingEdge) => attrs.push("edges=rising"),
        Some(EdgeDetection::FallingEdge) => attrs.push("edges=falling"),
        Some(EdgeDetection::BothEdges) => attrs.push("edges=both"),
    }
    match li.event_clock {
        None => (),                        // Not present for v1.
        Some(EventClock::Monotonic) => (), // default for ABI v2
        Some(EventClock::Realtime) => attrs.push("event-clock=realtime"),
        Some(EventClock::Hte) => attrs.push("event-clock=hte"),
    }
    let db;
    if li.debounce_period.is_some() {
        db = format!("debounce-period={:?}", li.debounce_period.unwrap());
        attrs.push(&db);
    }
    let consumer;
    if li.used {
        if li.consumer.is_empty() {
            consumer = String::from("consumer=kernel");
        } else {
            consumer = format!("consumer={}", li.consumer);
        }
        attrs.push(&consumer);
    }
    attrs.join(" ")
}

pub fn print_line_name(ci: &ChipInfo, offset: &Offset) {
    if let Some(name) = ci.named_lines.get(offset) {
        print!("{}", name);
    } else {
        print!("unnamed");
    }
}

pub fn print_consumer(li: &Info) {
    if li.used {
        if li.consumer.is_empty() {
            print!("kernel");
        } else {
            print!("{}", li.consumer);
        }
    } else {
        print!("unused");
    }
}

pub enum TimeFmt {
    Seconds,
    Localtime,
    Utc,
}

pub fn print_time(evtime: u64, timefmt: &TimeFmt) {
    use chrono::{Local, NaiveDateTime, TimeZone, Utc};

    let ts_sec = (evtime / 1000000000) as i64;
    let ts_nsec = (evtime % 1000000000) as u32;
    match timefmt {
        TimeFmt::Seconds => print!("{}.{}", ts_sec, ts_nsec),
        TimeFmt::Localtime => {
            let t = Local.from_utc_datetime(&NaiveDateTime::from_timestamp(ts_sec, ts_nsec));
            print!("{}", t.format("%FT%T%.9f"));
        }
        TimeFmt::Utc => {
            let t = Utc.from_utc_datetime(&NaiveDateTime::from_timestamp(ts_sec, ts_nsec));
            print!("{}", t.format("%FT%T%.9fZ"));
        }
    }
}

#[derive(Debug)]
struct NonUniqueLineError {
    id: String,
}

impl NonUniqueLineError {
    pub fn new<S: Into<String>>(name: S) -> NonUniqueLineError {
        NonUniqueLineError { id: name.into() }
    }
}

impl fmt::Display for NonUniqueLineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "line '{}' is not unique", self.id)
    }
}
impl Error for NonUniqueLineError {}

#[derive(Debug)]
struct RepeatedLineError {
    first: String,
    second: String,
}

impl RepeatedLineError {
    pub fn new<S: Into<String>>(first: S, second: S) -> RepeatedLineError {
        RepeatedLineError {
            first: first.into(),
            second: second.into(),
        }
    }
}

impl fmt::Display for RepeatedLineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "lines '{}' and '{}' are the same line",
            self.first, self.second
        )
    }
}
impl Error for RepeatedLineError {}

#[derive(Debug)]
struct LineNotFoundError {
    id: String,
}

impl LineNotFoundError {
    pub fn new<S: Into<String>>(id: S) -> LineNotFoundError {
        LineNotFoundError { id: id.into() }
    }
}

impl fmt::Display for LineNotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "cannot find line '{}'", self.id)
    }
}
impl Error for LineNotFoundError {}

#[derive(Debug)]
struct OffsetOutOfRangeError {
    id: String,
    chip_id: String,
}

impl OffsetOutOfRangeError {
    pub fn new<S: Into<String>>(id: S, chip_id: S) -> OffsetOutOfRangeError {
        OffsetOutOfRangeError {
            id: id.into(),
            chip_id: chip_id.into(),
        }
    }
}

impl fmt::Display for OffsetOutOfRangeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "offset {} is out of range on chip '{}'",
            self.id, self.chip_id
        )
    }
}
impl Error for OffsetOutOfRangeError {}

#[cfg(test)]
mod tests {
    use super::*;

    mod parse {
        #[test]
        fn duration() {
            use super::{parse_duration, ParseDurationError};
            use std::time::Duration;

            assert_eq!(parse_duration("0").unwrap(), Duration::ZERO);
            assert_eq!(parse_duration("1").unwrap(), Duration::from_millis(1));
            assert_eq!(parse_duration("2ms").unwrap(), Duration::from_millis(2));
            assert_eq!(parse_duration("3us").unwrap(), Duration::from_micros(3));
            assert_eq!(parse_duration("4s").unwrap(), Duration::new(4, 0));
            assert_eq!(
                parse_duration("5ns").unwrap_err(),
                ParseDurationError::Units("5ns".to_string())
            );
            assert_eq!(
                parse_duration("bad").unwrap_err(),
                ParseDurationError::NoDigits("bad".to_string())
            );
        }
    }
}
