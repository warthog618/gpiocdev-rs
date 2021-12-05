// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{all_chips, parse_chip_path};
use anyhow::{Context, Result};
use gpiod::chip::Chip;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Opts {
    /// The chips to interrogate.  If not specified then all chips are searched.
    #[structopt(parse(from_os_str = parse_chip_path))]
    chips: Vec<PathBuf>,
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let chips = if opts.chips.is_empty() {
        all_chips()?
    } else {
        opts.chips.clone()
    };
    for p in chips {
        detect_chip(&p)?;
    }
    Ok(())
}

fn detect_chip(p: &Path) -> Result<()> {
    let mut chip =
        Chip::from_path(&p).with_context(|| format!("Failed to open GPIO chip {:?}.", &p))?;
    print_chip_info(&mut chip)
}

fn print_chip_info(chip: &mut Chip) -> Result<()> {
    let ci = chip
        .chip_info()
        .with_context(|| format!("Failed to read chip {:?} info.", chip.path))?;
    println!(
        "{} [{}] ({} lines)",
        ci.name.to_string_lossy(),
        ci.label.to_string_lossy(),
        ci.num_lines
    );
    Ok(())
}
