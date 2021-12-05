// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{all_chips, chip_from_opts, parse_chip_path, string_or_default, UapiOpts};
use anyhow::{Context, Result};
use gpiod::chip::Chip;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Opts {
    /// The chips to interrogate.  If not specified then all chips are searched.
    #[structopt(short, long, parse(from_os_str = parse_chip_path))]
    chips: Vec<PathBuf>,
    /// The name of the line to find.
    #[structopt()]
    line: String,
    #[structopt(flatten)]
    uapi_opts: UapiOpts,
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let chips = if opts.chips.is_empty() {
        all_chips()?
    } else {
        opts.chips.clone()
    };
    let mut found = false;
    for p in chips {
        let mut c = chip_from_opts(&p, opts.uapi_opts.abiv)?;
        if find_line(&mut c, &opts.line)? {
            found = true;
        }
    }
    if !found {
        println!("Line \"{}\" not found.", &opts.line);
    }
    Ok(())
}

// Exhaustive form that checks every line even when a matching line has already been found.
fn find_line(chip: &mut Chip, line: &str) -> Result<bool> {
    let ci = chip
        .chip_info()
        .with_context(|| format!("Failed to read chip {:?} info.", chip.path))?;
    let mut found = false;
    for offset in 0..ci.num_lines {
        let li = chip.line_info(offset).with_context(|| {
            format!(
                "Failed to read line {} info from chip {:?}.",
                offset, chip.path
            )
        })?;
        if li.name.as_os_str() == line {
            println!(
                "{} line {:>3}:{:>12}{:>12}",
                chip.name().to_string_lossy(),
                li.offset,
                &li.name.to_string_lossy(),
                string_or_default(&li.consumer.to_string_lossy(), "unused"),
            );
            found = true;
        }
    }
    Ok(found)
}
