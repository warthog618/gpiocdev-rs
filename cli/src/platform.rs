// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use anyhow::Result;
use clap::Parser;
use gpiocdev::AbiVersion;
use std::fs;

#[derive(Debug, Parser)]
#[command(aliases(["p"]))]
pub struct Opts {
    #[arg(from_global)]
    pub verbose: bool,
}

pub fn cmd(opts: &Opts) -> Result<()> {
    if let Ok(v) = fs::read_to_string("/proc/version") {
        let mut f = v.split_ascii_whitespace();
        if let Some(v) = f.nth(2) {
            println!("Kernel {}", v);
            print_abi_support();
            if opts.verbose {
                print_unsupported_features(v)
            }
        }
    } else {
        println!("Kernel unknown");
        print_abi_support();
    }
    Ok(())
}

fn print_abi_support() {
    let versions = [AbiVersion::V1, AbiVersion::V2];
    for v in versions {
        match gpiocdev::supports_abi_version(v) {
            Err(gpiocdev::Error::NoGpioChips()) => {
                println!("No available gpiochips");
                return;
            }
            Ok(()) => println!("{} is supported.", v),
            Err(err) => println!("{}", err),
        }
    }
}

fn print_unsupported_features(version: &str) {
    if let Some((major, minor)) = parse_version(version) {
        if major >= 6 {
            println!("Kernel supports all uAPI features.");
        } else if major == 5 && minor > 10 {
            println!("Kernel does not support HTE.");
        } else if major < 5 || (major == 5 && minor < 5) {
            println!("Kernel does not support bias or reconfigure.");
        }
    }
}

fn parse_version(version: &str) -> Option<(u32, u32)> {
    let mut f = version.split('.');
    let major = f.next()?.parse().ok()?;
    let minor = f.next()?.parse().ok()?;
    Some((major, minor))
}
