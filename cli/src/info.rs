// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{self, LineOpts, UapiOpts};
use anyhow::{Context, Result};
use clap::Parser;
use gpiocdev::chip::Chip;
use gpiocdev::line::{Info, Offset};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(alias("i"))]
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
    #[arg(short, long, name = "chip", value_parser = common::parse_chip_path, verbatim_doc_comment)]
    chip: Option<PathBuf>,

    /// Lines are strictly identified by name.
    ///
    /// If --chip is provided then lines are initially assumed to be offsets, and only
    /// fallback to names if the line does not parse as an offset.
    ///
    /// With --by-name set the lines are never assumed to be identified by offsets, only names.
    #[arg(long)]
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
    if !(opts.lines.is_empty() || opts.strict) {
        return print_first_matching_lines(opts);
    }

    let chips = if let Some(p) = &opts.chip {
        vec![p.clone()]
    } else {
        common::all_chips()?
    };
    for p in chips {
        let mut c = common::chip_from_opts(&p, opts.uapi_opts.abiv)?;
        if opts.lines.is_empty() {
            print_chip_all_lines(&mut c)?;
        } else {
            print_chip_matching_lines(&mut c, opts)?;
        }
    }
    Ok(())
}

fn print_chip_all_lines(chip: &mut Chip) -> Result<()> {
    let kci = chip
        .info()
        .with_context(|| format!("Failed to read info from {}.", chip.name()))?;
    println!(
        "{} - {} lines",
        common::string_or_default(&kci.name, "??"),
        kci.num_lines
    );
    for offset in 0..kci.num_lines {
        let li = chip.line_info(offset).with_context(|| {
            format!(
                "Failed to read line {} info from chip {:?}.",
                offset,
                chip.name()
            )
        })?;
        println!(
            "\tline {:>3}:\t{:16}\t{}",
            li.offset,
            common::string_or_default(&li.name, "unnamed"),
            common::stringify_attrs(&li),
        );
    }
    Ok(())
}

fn print_chip_matching_lines(c: &mut Chip, opts: &Opts) -> Result<()> {
    let kci = c
        .info()
        .with_context(|| format!("Failed to read info from {}.", c.name()))?;
    for offset in 0..kci.num_lines {
        let li = c
            .line_info(offset)
            .with_context(|| format!("Failed to read line {} info from {}.", offset, c.name()))?;
        if opts.lines.contains(&li.name) {
            print_line_info(&kci.name, li);
        } else if !opts.by_name && opts.chip.is_some() {
            let name = format!("{}", offset);
            if opts.lines.contains(&name) {
                print_line_info(&kci.name, li);
            }
        }
    }
    Ok(())
}

fn print_first_matching_lines(opts: &Opts) -> Result<()> {
    let line_opts = LineOpts {
        chip: opts.chip.clone(),
        strict: false,
        by_name: opts.by_name,
    };
    let r = common::resolve_lines(&opts.lines, &line_opts, opts.uapi_opts.abiv)?;
    for (idx, ci) in r.chips.iter().enumerate() {
        let mut offsets: Vec<Offset> = r
            .lines
            .values()
            .filter(|co| co.chip_idx == idx)
            .map(|co| co.offset)
            .collect();
        offsets.sort_unstable();
        let mut c = common::chip_from_opts(&ci.path, opts.uapi_opts.abiv)?;
        print_chip_line_info(&mut c, &offsets)?;
    }
    Ok(())
}

fn print_chip_line_info(chip: &mut Chip, lines: &[Offset]) -> Result<()> {
    let kci = chip
        .info()
        .with_context(|| format!("Failed to read info from {}.", chip.name()))?;
    for &offset in lines {
        let li = chip.line_info(offset).with_context(|| {
            format!(
                "Failed to read line {} info from chip {:?}.",
                offset,
                chip.path()
            )
        })?;
        print_line_info(&kci.name, li);
    }
    Ok(())
}

fn print_line_info(chip_name: &str, li: Info) {
    println!(
        "{} {}\t{:16}\t{}",
        common::string_or_default(chip_name, "??"),
        li.offset,
        common::string_or_default(&li.name, "unnamed"),
        common::stringify_attrs(&li),
    );
}
