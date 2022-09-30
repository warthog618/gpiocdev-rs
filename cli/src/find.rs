// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    all_chips, chip_from_opts, parse_chip_path, string_or_default, stringify_attrs, UapiOpts,
};
use anyhow::{Context, Result};
use clap::Parser;
use gpiocdev::chip::Chip;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct Opts {
    /// Only search for the lines on this chip.
    ///
    /// If not specified the named line is searched for on all chips in the system.
    ///
    /// The chip may be identified by number, name, or path.
    /// e.g. the following all select the same chip:
    ///     -c 0
    ///     -c gpiochip0
    ///     -c /dev/gpiochip0
    #[arg(short, long, name = "chip", value_parser = parse_chip_path, verbatim_doc_comment)]
    chip: Option<PathBuf>,
    /// The name of the line to find.
    #[arg(name = "line")]
    line: String,
    /// Check all lines - don't assume names are unique.
    ///
    /// If not specified then the command stops when a matching line is found.
    ///
    /// If specified then all lines with the specified name are returned,
    /// each on a separate line.
    #[arg(short = 's', long)]
    strict: bool,
    /// Print the info for found lines.
    #[arg(short, long)]
    pub info: bool,
    #[command(flatten)]
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
            if !opts.strict {
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

// strict form that checks every line even when a matching line has already been found.
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
        if li.name.as_str() == opts.line.as_str() {
            if opts.info {
                println!(
                    "{} {}\t{}\t{} [{}]",
                    chip.name(),
                    li.offset,
                    &li.name,
                    string_or_default(&li.consumer, "unused"),
                    stringify_attrs(&li),
                );
            } else {
                println!("{} {}", chip.name(), li.offset);
            }
            if !opts.strict {
                return Ok(true);
            }
            found = true;
        }
    }
    Ok(found)
}
