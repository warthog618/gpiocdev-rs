// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::common::{self, format_error, EmitOpts};
use anyhow::Result;
use clap::Parser;
use gpiocdev::chip::Info;
#[cfg(feature = "serde")]
use serde_derive::Serialize;
use std::path::Path;

#[derive(Debug, Parser)]
#[command(aliases(["c", "detect"]))]
pub struct Opts {
    /// The chips to report
    ///
    /// If none specified then all chips are returned.
    ///
    /// Chips may be identified by number, name, or path.
    /// e.g. the following all select the same chip:
    ///     0
    ///     gpiochip0
    ///     /dev/gpiochip0
    #[arg(value_name = "chip", verbatim_doc_comment)]
    chips: Vec<String>,

    #[command(flatten)]
    emit: common::EmitOpts,
}

pub fn cmd(opts: &Opts) -> bool {
    let mut res = CmdResult {
        opts: opts.emit,
        ..Default::default()
    };
    if opts.chips.is_empty() {
        match &common::all_chip_paths() {
            Ok(pp) => {
                for p in pp {
                    res.push(chip_info(p));
                }
            }
            Err(e) => {
                res.push_error(e);
            }
        }
    } else {
        for id in &opts.chips {
            res.push(chip_info_from_id(id));
        }
    };
    res.emit();
    res.errors.is_empty()
}

// report error and fail overall operation if id does not correspond to a gpiochip.
fn chip_info_from_id(id: &str) -> Result<Info> {
    chip_info(&common::chip_lookup_from_id(id)?)
}

fn chip_info(p: &Path) -> Result<Info> {
    Ok(common::chip_from_path(p, gpiocdev::AbiVersion::V2)?.info()?)
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(Serialize))]
struct CmdResult {
    #[cfg_attr(feature = "serde", serde(skip))]
    opts: EmitOpts,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    chips: Vec<Info>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    errors: Vec<String>,
}

impl CmdResult {
    fn push(&mut self, r: Result<Info>) {
        match r {
            Ok(i) => self.chips.push(i),
            Err(e) => self.push_error(&e),
        }
    }
    fn push_error(&mut self, e: &anyhow::Error) {
        self.errors.push(format_error(&self.opts, e));
    }

    fn emit(&self) {
        #[cfg(feature = "json")]
        if self.opts.json {
            println!("{}", serde_json::to_string(&self).unwrap());
            return;
        }
        self.print()
    }

    fn print(&self) {
        for ci in &self.chips {
            println!(
                "{} [{}] ({} lines)",
                common::format_chip_name(&ci.name),
                ci.label,
                ci.num_lines
            );
        }
        for e in &self.errors {
            eprintln!("{e}");
        }
    }
}
