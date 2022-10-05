// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::common;
use anyhow::{Context, Result};
use clap::Parser;
use gpiocdev::chip::Chip;

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
    #[arg(name = "chip", verbatim_doc_comment)]
    chips: Vec<String>,

    #[arg(from_global)]
    pub verbose: bool,
}

pub fn cmd(opts: &Opts) -> Result<()> {
    if opts.chips.is_empty() {
        for p in common::all_chips()? {
            match Chip::from_path(&p) {
                Err(e) => {
                    if opts.verbose {
                        eprintln!("unable to open {}: {:?}", p.display(), e);
                    } else {
                        eprintln!("unable to open {}: {:#}", p.display(), e);
                    }
                }
                Ok(mut c) => {
                    if opts.verbose {}
                    if let Err(e) = print_chip_info(&mut c) {
                        if opts.verbose {
                            eprintln!("unable to read info for {}: {:?}", c.name(), e);
                        } else {
                            eprintln!("unable to read info for {}: {:#}", c.name(), e);
                        }
                    }
                }
            }
        }
    } else {
        for id in &opts.chips {
            let mut c = Chip::from_path(common::chip_path_from_id(id))
                .with_context(|| format!("cannot find GPIO chip character device '{}'", id))?;
            print_chip_info(&mut c)
                .with_context(|| format!("unable to read info for {}", c.name()))?;
        }
    }
    Ok(())
}

fn print_chip_info(chip: &mut Chip) -> Result<()> {
    let ci = chip.info()?;
    println!("{} [{}] ({} lines)", ci.name, ci.label, ci.num_lines);
    Ok(())
}
