// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::common::EmitOpts;
use clap::Parser;
use core::fmt;
use gpiocdev::{AbiSupportKind, AbiVersion};
#[cfg(feature = "serde")]
use serde_derive::Serialize;
use std::fs;
#[derive(Debug, Parser)]
#[command(aliases(["p"]))]
pub struct Opts {
    #[command(flatten)]
    emit: EmitOpts,
}

pub fn cmd(opts: &Opts) -> bool {
    let mut p = Platform {
        opts: opts.emit,
        ..Default::default()
    };
    if let Ok(v) = fs::read_to_string("/proc/version") {
        let mut f = v.split_ascii_whitespace();
        if let Some(v) = f.nth(2) {
            p.kernel.version = v.into();
        }
    } else {
        p.kernel.version = "unknown".into();
    }
    let versions = [AbiVersion::V1, AbiVersion::V2];
    for v in versions {
        match Abi::new(v, &p.kernel.version) {
            Ok(abi) => {
                p.abis.push(abi);
            }
            Err(e) => {
                p.errors.push(e.to_string());
                break;
            }
        }
    }
    p.emit();
    p.errors.is_empty()
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(Serialize))]
struct Platform {
    #[cfg_attr(feature = "serde", serde(skip))]
    opts: EmitOpts,
    kernel: Kernel,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    abis: Vec<Abi>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    errors: Vec<String>,
}

impl Platform {
    fn emit(&self) {
        #[cfg(feature = "json")]
        if self.opts.json {
            println!(
                "{}",
                serde_json::to_string(self).expect("serialize should succeed")
            );
            return;
        }
        self.print();
    }

    fn print(&self) {
        println!("Kernel {}", self.kernel.version);
        for e in &self.errors {
            eprintln!("{e}");
        }
        for abi in &self.abis {
            match abi.unsupported_by {
                None => println!("{} is supported.", abi.version),
                Some(k) => println!("{}", gpiocdev::Error::UnsupportedAbi(abi.version, k)),
            }
            if self.opts.verbose {
                if abi.unsupported_by.is_none() && abi.missing_features.is_empty() {
                    println!("Kernel supports all {} features.", abi.version);
                }
                for f in &abi.missing_features {
                    println!("Kernel version does not support {f}");
                }
            }
        }
    }
}

fn parse_version(version: &str) -> Option<(u32, u32)> {
    let mut f = version.split('.');
    let major = f.next()?.parse().ok()?;
    let minor = f.next()?.parse().ok()?;
    Some((major, minor))
}

#[cfg_attr(feature = "serde", derive(Serialize), serde(rename_all = "lowercase"))]
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
        write!(f, "{msg}")
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(Serialize))]
struct Kernel {
    version: String,
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(Serialize), serde(rename_all = "camelCase"))]
struct Abi {
    version: AbiVersion,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    unsupported_by: Option<AbiSupportKind>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    missing_features: Vec<Features>,
}

impl Abi {
    fn new(v: AbiVersion, kernel: &str) -> gpiocdev::Result<Abi> {
        let mut abi = Abi {
            version: v,
            ..Default::default()
        };
        if let Some((major, minor)) = parse_version(kernel) {
            match v {
                AbiVersion::V1 => {
                    if major < 5 || (major == 5 && minor < 5) {
                        abi.missing_features.push(Features::Bias);
                    }
                }
                AbiVersion::V2 => {
                    if major < 5 || (major == 5 && minor <= 10) {
                        abi.missing_features.push(Features::Realtime);
                    }
                    if major < 5 || (major == 5 && minor < 19) {
                        abi.missing_features.push(Features::Hte);
                    }
                }
            }
        }
        if let Err(e) = gpiocdev::supports_abi_version(v) {
            if let gpiocdev::Error::UnsupportedAbi(_v, k) = e {
                abi.unsupported_by = Some(k)
            } else {
                return Err(e);
            }
        }
        Ok(abi)
    }
}
