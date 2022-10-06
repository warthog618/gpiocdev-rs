// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::common;
use anyhow::Result;
use clap::Parser;
use gpiocdev::chip::Chip;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(aliases(["c","detect"]))]
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
    #[arg(name = "chip", verbatim_doc_comment)]
    chips: Vec<String>,

    #[arg(from_global)]
    pub verbose: bool,
}

// report error and fail overall operation if id does not correspond to a gpiochip.
fn chip_lookup_from_id(id: &str, success: &mut bool) -> Option<PathBuf> {
    match common::chip_lookup_from_id(id) {
        Ok(p) => return Some(p),
        Err(e) => {
            eprintln!("{}", e);
            *success = false;
        }
    }
    None
}

pub fn cmd(opts: &Opts) -> Result<bool> {
    let mut success = true;

    let chips = if opts.chips.is_empty() {
        common::all_chip_paths()?
    } else {
        opts.chips
            .iter()
            .filter_map(|id| chip_lookup_from_id(id, &mut success))
            .collect()
    };

    for p in chips {
        if !print_chip_info(&p, opts.verbose)? {
            success = false;
        }
    }
    Ok(success)
}

fn print_chip_info(p: &PathBuf, verbose: bool) -> Result<bool> {
    match Chip::from_path(p) {
        Ok(c) => {
            let ci = c.info()?;
            println!("{} [{}] ({} lines)", ci.name, ci.label, ci.num_lines);
            return Ok(true);
        }
        Err(e) if verbose => eprintln!("unable to open '{}': {:#}", p.display(), e),
        Err(_) => eprintln!("unable to open '{}'", p.display()),
    }
    Ok(false)
}
