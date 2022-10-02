// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::common;
use anyhow::{Context, Result};
use clap::Parser;
use gpiocdev::chip::Chip;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(alias("d"))]
pub struct Opts {
    /// The chips to detect.
    ///
    /// If not specified then all chips are returned.
    ///
    /// Chips may be identified by number, name, or path.
    /// e.g. the following all select the same chip:
    ///     0
    ///     gpiochip0
    ///     /dev/gpiochip0
    #[arg(name = "chip", value_parser = common::parse_chip_path, verbatim_doc_comment)]
    chips: Vec<PathBuf>,
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let chips = if opts.chips.is_empty() {
        common::all_chips()?
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
        .info()
        .with_context(|| format!("Failed to read chip {:?} info.", chip.path()))?;
    println!(
        "{} [{}] ({} lines)",
        ci.name,
        ci.label,
        ci.num_lines
    );
    Ok(())
}
