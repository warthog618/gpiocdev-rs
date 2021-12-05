// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use anyhow::{Context, Result};
use gpiod::chip::{chips, is_chip, Chip};
use gpiod::request::Config;
use gpiod::{
    detect_abi_version,
    AbiVersion::{self, *},
};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames, VariantNames};

// common helper functions

pub fn all_chips() -> Result<Vec<PathBuf>> {
    chips().context("Failed to find any chips.")
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
    // from name (most likely case)
    let mut p = OsString::from("/dev/");
    p.push(s);
    if let Ok(rp) = is_chip(p) {
        return rp;
    }
    // from number
    p = OsString::from("/dev/gpiochip");
    p.push(s);
    if let Ok(rp) = is_chip(p) {
        return rp;
    }
    // from raw path
    PathBuf::from(s)
}

#[derive(Debug, thiserror::Error)]
pub enum ParseDurationError {
    #[error("Unknown units '{0}'. Use \"s\", \"ms\", \"us\" or \"ns\".")]
    Units(String),
    #[error(transparent)]
    Digits(std::num::ParseIntError),
}

pub fn parse_duration(s: &str) -> std::result::Result<Duration, ParseDurationError> {
    let t = match s.find(|c: char| !c.is_ascii_digit()) {
        Some(n) => {
            let (num, units) = s.split_at(n);
            let t = num.parse::<u64>().map_err(ParseDurationError::Digits)?;
            let multiplier;
            match units {
                "ns" => multiplier = 1,
                "us" => multiplier = 1000,
                "ms" => multiplier = 1000000,
                "s" => multiplier = 1000000000,
                _ => return Err(ParseDurationError::Units(units.to_string())),
            }
            t * multiplier
        }
        None => s.parse::<u64>().map_err(ParseDurationError::Digits)?,
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

#[derive(Debug, StructOpt)]
pub struct UapiOpts {
    /// The uAPI ABI version to use to perform the operation.
    ///
    /// The auto option detects the uAPI versions supported by the kernel and uses the latest.
    // This is primarily aimed at debugging and so is a hidden option.
    #[structopt(long, default_value = "0", hidden = true, env = "GPIOD_ABI_VERSION")]
    pub abiv: u8,
}

#[derive(Debug, StructOpt)]
pub struct ActiveLowOpts {
    /// Treat the line as active-low when determining value.
    #[structopt(short = "l", long)]
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
impl From<BiasFlags> for gpiod::line::Bias {
    fn from(b: BiasFlags) -> Self {
        match b {
            BiasFlags::PullUp => gpiod::line::Bias::PullUp,
            BiasFlags::PullDown => gpiod::line::Bias::PullDown,
            BiasFlags::Disabled => gpiod::line::Bias::Disabled,
        }
    }
}

#[derive(Copy, Clone, Debug, StructOpt)]
pub struct BiasOpts {
    /// The bias to be applied to the lines.
    #[structopt(short, long, possible_values = BiasFlags::VARIANTS, case_insensitive = true)]
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
impl From<DriveFlags> for gpiod::line::Drive {
    fn from(b: DriveFlags) -> Self {
        match b {
            DriveFlags::PushPull => gpiod::line::Drive::PushPull,
            DriveFlags::OpenDrain => gpiod::line::Drive::OpenDrain,
            DriveFlags::OpenSource => gpiod::line::Drive::OpenSource,
        }
    }
}
#[derive(Copy, Clone, Debug, StructOpt)]
pub struct DriveOpts {
    /// How the lines should be driven.
    #[structopt(short, long, possible_values = DriveFlags::VARIANTS, case_insensitive = true)]
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
impl From<EdgeFlags> for gpiod::line::EdgeDetection {
    fn from(b: EdgeFlags) -> Self {
        match b {
            EdgeFlags::RisingEdge => gpiod::line::EdgeDetection::RisingEdge,
            EdgeFlags::FallingEdge => gpiod::line::EdgeDetection::FallingEdge,
            EdgeFlags::BothEdges => gpiod::line::EdgeDetection::BothEdges,
        }
    }
}
#[derive(Copy, Clone, Debug, StructOpt)]
pub struct EdgeOpts {
    /// Which edges should be detected and reported.
    #[structopt(short, long, possible_values = EdgeFlags::VARIANTS, default_value="both-edges", case_insensitive = true)]
    pub edge: EdgeFlags,
}
impl EdgeOpts {
    pub fn apply(self, r: &mut Config) -> &mut Config {
        r.with_edge_detection(Some(self.edge.into()))
    }
}
