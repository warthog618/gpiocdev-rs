// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::common::{self, emit_error};
use anyhow::Result;
use clap::Parser;
use gpiocdev::chip::Info;
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
    let mut info = Vec::new();
    if opts.chips.is_empty() {
        match &common::all_chip_paths() {
            Ok(pp) => {
                for p in pp {
                    info.push(chip_info(p));
                }
            }
            Err(e) => {
                emit_error(&opts.emit, e);
                return false;
            }
        }
    } else {
        for id in &opts.chips {
            info.push(chip_info_from_id(id));
        }
    };
    emit_chip_info(&info, &opts.emit)
}

// report error and fail overall operation if id does not correspond to a gpiochip.
fn chip_info_from_id(id: &str) -> Result<Info> {
    chip_info(&common::chip_lookup_from_id(id)?)
}

fn chip_info(p: &Path) -> Result<Info> {
    Ok(common::chip_from_path(p, gpiocdev::AbiVersion::V2)?.info()?)
}

fn emit_chip_info(info: &Vec<Result<Info>>, opts: &common::EmitOpts) -> bool {
    print_chip_info(info, opts)
}

fn print_chip_info(info: &Vec<Result<Info>>, opts: &common::EmitOpts) -> bool {
    let mut success = true;
    for i in info {
        match i {
            Ok(ci) => {
                println!(
                    "{} [{}] ({} lines)",
                    common::format_chip_name(&ci.name),
                    ci.label,
                    ci.num_lines
                );
            }
            Err(e) => {
                emit_error(opts, e);
                success = false;
            }
        }
    }
    success
}
