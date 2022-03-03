// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    all_chips, chip_from_opts, parse_chip_path, string_or_default, stringify_attrs, UapiOpts,
};
use anyhow::{Context, Result};
use clap::Parser;
use gpiod::chip::Chip;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct Opts {
    /// Restrict the search to lines on this chip.
    /// If omitted then all chips are searched.
    #[clap(short, long, parse(from_os_str = parse_chip_path))]
    chip: Option<PathBuf>,
    /// The name of the line to find.
    #[clap()]
    line: String,
    /// Check all lines - don't assume names are unique.
    #[clap(short = 'X', long)]
    exhaustive: bool,
    /// Print the info for found lines.
    #[clap(short, long)]
    pub info: bool,
    #[clap(flatten)]
    uapi_opts: UapiOpts,
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let chips = match &opts.chip {
        None => all_chips()?,
        Some(chip) => vec![chip.clone()],
    };
    let mut found = false;
    for p in chips {
        let mut c = chip_from_opts(&p, opts.uapi_opts.abiv)?;
        if find_line(&mut c, opts)? {
            if !opts.exhaustive {
                return Ok(());
            }
            found = true;
        }
    }
    if !found {
        println!("Can't find line {:?}.", &opts.line);
    }
    Ok(())
}

// Exhaustive form that checks every line even when a matching line has already been found.
fn find_line(chip: &mut Chip, opts: &Opts) -> Result<bool> {
    let ci = chip
        .info()
        .with_context(|| format!("Failed to read chip {:?} info.", chip.path()))?;
    let mut found = false;
    for offset in 0..ci.num_lines {
        let li = chip.line_info(offset).with_context(|| {
            format!(
                "Failed to read line {} info from chip {:?}.",
                offset,
                chip.path()
            )
        })?;
        if li.name.as_os_str() == opts.line.as_str() {
            if opts.info {
                println!(
                    "{} {}\t{}\t{} [{}]",
                    chip.name().to_string_lossy(),
                    li.offset,
                    &li.name.to_string_lossy(),
                    string_or_default(&li.consumer.to_string_lossy(), "unused"),
                    stringify_attrs(&li),
                );
            } else {
                println!("{} {}", chip.name().to_string_lossy(), li.offset);
            }
            if !opts.exhaustive {
                return Ok(true);
            }
            found = true;
        }
    }
    Ok(found)
}
