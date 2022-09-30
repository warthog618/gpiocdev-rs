// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    all_chips, chip_from_opts, find_lines, parse_chip_path, string_or_default, stringify_attrs,
    LineOpts, UapiOpts,
};
use anyhow::{Context, Result};
use clap::Parser;
use gpiocdev::chip::Chip;
use gpiocdev::line::Offset;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct Opts {
    /// Only get information for the specified lines.
    ///
    /// The lines are identified by name or optionally by offset
    /// if the --chip option is provided.
    ///
    /// If not specified then all lines are returned.
    #[arg(name = "line")]
    lines: Vec<String>,
    /// Restrict scope to the lines on this chip.
    ///
    /// If not specified then the scope is all chips in the system.
    ///
    /// If specified then lines may be identified by either name or offset.
    ///
    /// The chip may be identified by number, name, or path.
    /// e.g. the following all select the same chip:
    ///     -c 0
    ///     -c gpiochip0
    ///     -c /dev/gpiochip0
    #[arg(short, long, name = "chip", value_parser = parse_chip_path, verbatim_doc_comment)]
    chip: Option<PathBuf>,
    /// Lines are strictly identified by name.
    ///
    /// If --chip is provided then lines are initially assumed to be offsets, and only
    /// fallback to names if the line does not parse as an offset.
    ///
    /// With --by-name set the lines are never assumed to be identified by offsets, only names.
    #[arg(short = 'N', long)]
    by_name: bool,
    /// Check all lines - don't assume names are unique.
    ///
    /// If not specified then the command stops when a matching line is found.
    ///
    /// If specified then all lines with the specified name are returned,
    /// each on a separate line.
    #[arg(short = 's', long)]
    strict: bool,
    #[command(flatten)]
    uapi_opts: UapiOpts,
}

pub fn cmd(opts: &Opts) -> Result<()> {
    if opts.lines.is_empty() {
        if let Some(p) = &opts.chip {
            let mut c = chip_from_opts(p, opts.uapi_opts.abiv)?;
            print_chip_all_line_info(&mut c)?;
        } else {
            for p in all_chips()? {
                let mut c = chip_from_opts(&p, opts.uapi_opts.abiv)?;
                print_chip_all_line_info(&mut c)?;
            }
        }
    } else {
        let line_opts = LineOpts {
            chip: opts.chip.to_owned(),
            strict: opts.strict,
            by_name: opts.by_name,
        };
        let (lines, mut chips) = find_lines(&opts.lines, &line_opts, opts.uapi_opts.abiv)?;
        chips.sort();
        for chip in chips {
            let mut offsets: Vec<Offset> = lines
                .values()
                .filter(|co| co.chip == *chip)
                .map(|co| co.offset)
                .collect();
            offsets.sort_unstable();
            let mut c = chip_from_opts(&chip, opts.uapi_opts.abiv)?;
            print_chip_line_info(&mut c, &offsets)?;
        }
    }
    Ok(())
}

fn print_chip_all_line_info(chip: &mut Chip) -> Result<()> {
    let ci = chip
        .info()
        .with_context(|| format!("Failed to read chip {:?} info.", chip.path()))?;
    println!(
        "{} - {} lines",
        string_or_default(&ci.name, "??"),
        ci.num_lines
    );
    for offset in 0..ci.num_lines {
        let li = chip.line_info(offset).with_context(|| {
            format!(
                "Failed to read line {} info from chip {:?}.",
                offset,
                chip.path()
            )
        })?;
        println!(
            "\tline {:>3}:\t{:16}\t{}",
            li.offset,
            string_or_default(&li.name, "unnamed"),
            stringify_attrs(&li),
        );
        }
    Ok(())
}

fn print_chip_line_info(chip: &mut Chip, lines: &[Offset]) -> Result<()> {
    let ci = chip
        .info()
        .with_context(|| format!("Failed to read info from chip {:?}.", chip.path()))?;
    for &offset in lines {
        let li = chip.line_info(offset).with_context(|| {
            format!(
                "Failed to read line {} info from chip {:?}.",
                offset,
                chip.path()
            )
        })?;
        println!(
            "{} {}\t{:16}\t{}",
            string_or_default(&ci.name, "??"),
            li.offset,
            string_or_default(&li.name, "unnamed"),
            stringify_attrs(&li),
        );
        }
    Ok(())
}
