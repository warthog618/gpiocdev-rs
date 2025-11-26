// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod resolver;
pub use self::resolver::{ChipInfo, Resolver};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use gpiocdev::chip::{chips, is_chip, Chip};
use gpiocdev::line::{Bias, Drive, EdgeDetection};
use gpiocdev::request::Config;
use gpiocdev::AbiVersion;
use std::path::{Path, PathBuf};
use std::time::Duration;

// common helper functions

pub fn all_chip_paths() -> Result<Vec<PathBuf>> {
    chips().context("unable to find any chips")
}

#[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
pub fn chip_from_path(p: &Path, abiv: AbiVersion) -> Result<Chip> {
    let mut c =
        Chip::from_path(p).with_context(|| format!("unable to open chip '{}'", p.display()))?;
    c.using_abi_version(abiv);
    Ok(c)
}
#[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
pub fn chip_from_path(p: &Path, _abiv: AbiVersion) -> Result<Chip> {
    Chip::from_path(p).with_context(|| format!("unable to open chip '{}'", p.display()))
}

#[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
pub fn actual_abi_version(opts: &UapiOpts) -> Result<AbiVersion> {
    Ok(match opts.abi_version {
        Some(abiv) => {
            let abiv = abiv.into();
            gpiocdev::supports_abi_version(abiv)?;
            abiv
        }
        None => gpiocdev::detect_abi_version()?,
    })
}

#[cfg(not(feature = "uapi_v2"))]
pub fn actual_abi_version(_opts: &UapiOpts) -> Result<AbiVersion> {
    Ok(AbiVersion::V1)
}

#[cfg(not(feature = "uapi_v1"))]
pub fn actual_abi_version(_opts: &UapiOpts) -> Result<AbiVersion> {
    Ok(AbiVersion::V2)
}

fn chip_path_from_id(id: &str) -> PathBuf {
    if id.chars().all(char::is_numeric) {
        // from number
        return format!("/dev/gpiochip{id}").into();
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
    is_chip(chip_path_from_id(id))
        .with_context(|| format!("cannot find GPIO chip character device '{id}'"))
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum ParseDurationError {
    #[error("'{0}' unknown units - use 's', 'ms' or 'us'.")]
    Units(String),
    #[error("'{0}' must start with a digit")]
    NoDigits(String),
    #[error("'{0}' {1}")]
    ParseDigits(String, std::num::ParseIntError),
}

pub fn parse_duration(s: &str) -> std::result::Result<Duration, ParseDurationError> {
    if s == "0" {
        return Ok(Duration::ZERO);
    }
    let t = match s.find(|c: char| !c.is_ascii_digit()) {
        Some(0) => return Err(ParseDurationError::NoDigits(s.into())),
        Some(n) => {
            let (num, units) = s.split_at(n);
            let t = num
                .parse::<u64>()
                .map_err(|e| ParseDurationError::ParseDigits(num.into(), e))?;
            t * match units {
                "us" => 1000,
                "ms" => 1000000,
                "s" => 1000000000,
                _ => return Err(ParseDurationError::Units(s.into())),
            }
        }
        None => {
            s.parse::<u64>()
                .map_err(|e| ParseDurationError::ParseDigits(s.into(), e))?
                * 1000000
        }
    };
    Ok(Duration::from_nanos(t))
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
    #[arg(short, long, value_name = "chip", verbatim_doc_comment)]
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

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AbiVersionFlags {
    V1,
    V2,
}
impl From<AbiVersionFlags> for gpiocdev::AbiVersion {
    fn from(b: AbiVersionFlags) -> Self {
        match b {
            AbiVersionFlags::V1 => gpiocdev::AbiVersion::V1,
            AbiVersionFlags::V2 => gpiocdev::AbiVersion::V2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Parser)]
pub struct EmitOpts {
    #[arg(from_global)]
    pub verbose: bool,

    /// Emit output in JSON format
    #[cfg(feature = "json")]
    #[arg(long, group = "emit")]
    pub json: bool,

    /// Quote line and consumer names.
    #[arg(long)]
    pub quoted: bool,
}

pub fn emit_error(opts: &EmitOpts, e: &anyhow::Error) {
    let e_str = format_error(opts, e);
    #[cfg(feature = "json")]
    if opts.json {
        println!("{{\"error\":\"{e_str}\"}}");
        return;
    }
    eprintln!("{e_str}");
}

pub fn format_error(opts: &EmitOpts, e: &anyhow::Error) -> String {
    if opts.verbose {
        format!("{e:#}")
    } else {
        format!("{e}")
    }
}

#[derive(Debug, Default, Parser)]
pub struct UapiOpts {
    /// The uAPI ABI version to use to perform the operation
    ///
    /// By default the latest uAPI version supported by the kernel is used.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    #[arg(
        long,
        value_name = "version",
        env = "GPIOCDEV_ABI_VERSION",
        value_enum,
        ignore_case = true
    )]
    pub abi_version: Option<AbiVersionFlags>,
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
    #[arg(short, long, value_name = "bias", value_enum, ignore_case = true)]
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
        value_name = "drive",
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
        value_name = "edges",
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

pub fn stringify_attrs(li: &gpiocdev::line::Info, quoted: bool) -> String {
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
    if let Some(debounce_period) = li.debounce_period {
        db = format!("debounce-period={:?}", debounce_period);
        attrs.push(&db);
    }
    let consumer;
    if li.used {
        if li.consumer.is_empty() {
            consumer = String::from("consumer=kernel");
        } else if quoted {
            consumer = format!("consumer=\"{}\"", li.consumer);
        } else {
            consumer = format!("consumer={}", li.consumer);
        }
        attrs.push(&consumer);
    }
    attrs.join(" ")
}

pub enum TimeFmt {
    Seconds,
    Localtime,
    Utc,
}

pub fn format_chip_name(n: &str) -> &str {
    if n.is_empty() {
        "??"
    } else {
        n
    }
}

pub fn format_time(evtime: u64, timefmt: &TimeFmt) -> String {
    use chrono::{Local, TimeZone, Utc};

    let ts_sec = (evtime / 1000000000) as i64;
    let ts_nsec = (evtime % 1000000000) as u32;
    match timefmt {
        TimeFmt::Seconds => format!("{ts_sec}.{ts_nsec:09}"),
        TimeFmt::Localtime => {
            let t = Local
                .timestamp_opt(ts_sec, ts_nsec)
                .single()
                .expect("event time should be valid");
            format!("{}", t.format("%FT%T%.9f"))
        }
        TimeFmt::Utc => {
            let t = Utc
                .timestamp_opt(ts_sec, ts_nsec)
                .single()
                .expect("event time should be valid");
            format!("{}", t.format("%FT%T%.9fZ"))
        }
    }
}

/// Errors returned by cli functions.
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum Error {
    #[error("lines '{0}' and '{1}' are the same line")]
    DuplicateLine(String, String),

    #[error("line '{0}' is not unique")]
    NonUniqueLine(String),

    #[error("line '{0}' is repeated")]
    RepeatedLine(String),

    #[error("cannot find line '{0}'")]
    NoSuchLine(String),

    #[error("offset {0} is out of range on chip '{1}'")]
    OffsetOutOfRange(String, String),
}

#[cfg(test)]
mod tests {
    use super::*;

    mod parse {
        #[test]
        fn duration() {
            use super::{parse_duration, ParseDurationError};
            use std::time::Duration;

            assert_eq!(
                parse_duration("0").expect("duration should be valid"),
                Duration::ZERO
            );
            assert_eq!(
                parse_duration("1").expect("duration should be valid"),
                Duration::from_millis(1)
            );
            assert_eq!(
                parse_duration("2ms").expect("duration should be valid"),
                Duration::from_millis(2)
            );
            assert_eq!(
                parse_duration("3us").expect("duration should be valid"),
                Duration::from_micros(3)
            );
            assert_eq!(
                parse_duration("4s").expect("duration should be valid"),
                Duration::new(4, 0)
            );
            assert_eq!(
                parse_duration("5ns").expect_err("duration should be invalid"),
                ParseDurationError::Units("5ns".to_string())
            );
            assert_eq!(
                parse_duration("bad").expect_err("duration should be invalid"),
                ParseDurationError::NoDigits("bad".to_string())
            );
        }
    }
}
