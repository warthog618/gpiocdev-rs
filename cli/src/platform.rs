// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::common::{self, emit_error};
use anyhow::anyhow;
use clap::Parser;
use core::fmt;
use gpiocdev::AbiVersion;
use std::fs;

#[derive(Debug, Parser)]
#[command(aliases(["p"]))]
pub struct Opts {
    #[command(flatten)]
    emit: common::EmitOpts,
}

pub fn cmd(opts: &Opts) -> bool {
    let mut p = Platform {
        ..Default::default()
    };
    let versions = [AbiVersion::V1, AbiVersion::V2];
    for v in versions {
        if gpiocdev::supports_abi_version(v).is_ok() {
            p.abi_versions.push(v);
        }
    }
    if let Ok(v) = fs::read_to_string("/proc/version") {
        let mut f = v.split_ascii_whitespace();
        if let Some(v) = f.nth(2) {
            p.kernel.version = v.into();
        }
        if let Some((major, minor)) = parse_version(&v) {
            if p.abi_versions.contains(&AbiVersion::V1) && (major < 5 || (major == 5 && minor < 5))
            {
                p.kernel.unsupported_features.push(Features::Bias);
            }
            if p.abi_versions.contains(&AbiVersion::V2) {
                if major == 5 && minor == 10 {
                    p.kernel.unsupported_features.push(Features::Realtime);
                }
                if major == 5 && minor > 10 && minor < 19 {
                    p.kernel.unsupported_features.push(Features::Hte);
                }
            }
        }
    } else {
        p.kernel.version = "unknown".into();
    }
    emit_platform(&p, &opts.emit)
}

fn emit_platform(p: &Platform, opts: &common::EmitOpts) -> bool {
    print_platform(p, opts)
}

fn print_platform(p: &Platform, opts: &common::EmitOpts) -> bool {
    println!("Kernel {}", p.kernel.version);
    if p.abi_versions.is_empty() {
        emit_error(opts, &anyhow!(gpiocdev::detect_abi_version().unwrap_err()));
        return false;
    }
    for v in &p.abi_versions {
        println!("{} is supported.", v);
    }
    if opts.verbose {
        if p.kernel.unsupported_features.is_empty() {
            println!("Kernel supports all uAPI features.");
        }
        for f in &p.kernel.unsupported_features {
            println!("Kernel does not support {}", f);
        }
    }
    true
}

fn parse_version(version: &str) -> Option<(u32, u32)> {
    let mut f = version.split('.');
    let major = f.next()?.parse().ok()?;
    let minor = f.next()?.parse().ok()?;
    Some((major, minor))
}

enum Features {
    Bias,
    Realtime,
    Hte,
}

impl fmt::Display for Features {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Features::Bias => "bias or reconfigure (added in 5.5)",
            Features::Realtime => "event_clock realtime (added in 5.11)",
            Features::Hte => "event_clock hte (added in 5.19)",
        };
        write!(f, "{}", msg)
    }
}

#[derive(Default)]
struct Kernel {
    version: String,
    unsupported_features: Vec<Features>,
}
#[derive(Default)]
struct Platform {
    kernel: Kernel,
    abi_versions: Vec<AbiVersion>,
}
