// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::common::{
    self, actual_abi_version, chip_from_path, emit_error, format_chip_name, stringify_attrs,
};
use anyhow::{Context, Result};
use clap::Parser;
use gpiocdev::chip::Chip;
use gpiocdev::line::{Info, Offset};

#[derive(Debug, Parser)]
#[command(aliases(["l", "info"]))]
pub struct Opts {
    /// Only get information for the specified lines
    ///
    /// The lines are identified by name or optionally by offset
    /// if the --chip option is provided.
    ///
    /// If not specified then all lines are returned.
    #[arg(value_name = "line")]
    lines: Vec<String>,

    /// Restrict scope to the lines on this chip
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
    #[arg(short, long, value_name = "chip", verbatim_doc_comment)]
    chip: Option<String>,

    /// Lines are strictly identified by name
    ///
    /// If --chip is provided then lines are initially assumed to be offsets, and only
    /// fallback to names if the line does not parse as an offset.
    ///
    /// With --by-name set the lines are never assumed to be identified by offsets, only names.
    #[arg(long)]
    by_name: bool,

    /// Check all lines - don't assume names are unique
    ///
    /// If not specified then the command stops when a matching line is found.
    ///
    /// If specified then all lines with the specified name are returned,
    /// each on a separate line.
    #[arg(short = 's', long)]
    strict: bool,

    #[command(flatten)]
    uapi_opts: common::UapiOpts,

    /// Quote line and consumer names.
    #[arg(long)]
    quoted: bool,

    #[command(flatten)]
    emit: common::EmitOpts,
}

pub fn cmd(opts: &Opts) -> bool {
    match cmd_inner(opts) {
        Err(e) => {
            emit_error(&opts.emit, &e);
            false
        }
        Ok(x) => x,
    }
}

fn cmd_inner(opts: &Opts) -> Result<bool> {
    let mut success = true;

    if !(opts.lines.is_empty() || opts.strict) {
        return print_first_matching_lines(opts);
    }

    let chips = if let Some(id) = &opts.chip {
        vec![common::chip_lookup_from_id(id)?]
    } else {
        common::all_chip_paths()?
    };
    let abiv = actual_abi_version(&opts.uapi_opts)?;
    let mut counts = vec![0; opts.lines.len()];
    for p in chips {
        match chip_from_path(&p, abiv) {
            Ok(c) => {
                if opts.lines.is_empty() {
                    if !print_chip_all_lines(&c, opts)? {
                        success = false;
                    }
                } else {
                    print_chip_matching_lines(&c, opts, &mut counts)?;
                }
            }
            Err(e) if opts.chip.is_some() => return Err(e),
            Err(e) if opts.emit.verbose => emit_error(&opts.emit, &e),
            Err(_) => {}
        }
    }
    if !opts.lines.is_empty() {
        success = counts.iter().all(|n| *n == 1);
    }
    Ok(success)
}

fn print_chip_all_lines(c: &Chip, opts: &Opts) -> Result<bool> {
    let kci = c
        .info()
        .with_context(|| format!("unable to read info from {}", c.name()))?;
    println!("{} - {} lines:", format_chip_name(&kci.name), kci.num_lines);
    for offset in 0..kci.num_lines {
        let li = c
            .line_info(offset)
            .with_context(|| format!("unable to read line {} info from {}.", offset, c.name()))?;
        let lname = if li.name.is_empty() {
            "unnamed".to_string()
        } else if opts.quoted {
            format!("\"{}\"", li.name)
        } else {
            li.name.to_string()
        };
        println!(
            "\tline {:>3}:\t{:16}\t{}",
            li.offset,
            lname,
            stringify_attrs(&li, opts.quoted),
        );
    }
    Ok(true)
}

fn print_chip_matching_lines(c: &Chip, opts: &Opts, counts: &mut [u32]) -> Result<bool> {
    let kci = c
        .info()
        .with_context(|| format!("unable to read info from {}", c.name()))?;
    for offset in 0..kci.num_lines {
        let li = c.line_info(offset).with_context(|| {
            format!("unable to read info for line {} from {}", offset, c.name())
        })?;
        let oname = format!("{}", offset);
        for (idx, id) in opts.lines.iter().enumerate() {
            if (id == &li.name) || (!opts.by_name && opts.chip.is_some() && id == &oname) {
                counts[idx] += 1;
                print_line_info(&kci.name, &li, opts.quoted);
            }
        }
    }
    Ok(true)
}

fn print_first_matching_lines(opts: &Opts) -> Result<bool> {
    let line_opts = common::LineOpts {
        chip: opts.chip.clone(),
        strict: false,
        by_name: opts.by_name,
    };
    let abiv = actual_abi_version(&opts.uapi_opts)?;
    let r = common::Resolver::resolve_lines_unvalidated(&opts.lines, &line_opts, abiv)?;
    for (idx, ci) in r.chips.iter().enumerate() {
        let mut offsets: Vec<Offset> = r
            .lines
            .values()
            .filter(|co| co.chip_idx == idx)
            .map(|co| co.offset)
            .collect();
        offsets.sort_unstable();
        offsets.dedup();
        let c = chip_from_path(&ci.path, abiv)?;
        print_chip_line_info(&c, &offsets, opts.quoted)?;
    }
    r.validate(&opts.lines, &line_opts)?;
    Ok(true)
}

fn print_chip_line_info(chip: &Chip, lines: &[Offset], quoted: bool) -> Result<bool> {
    for &offset in lines {
        let li = chip
            .line_info(offset)
            .with_context(|| format!("unable to read line {} info from {}", offset, chip.name()))?;
        print_line_info(&chip.name(), &li, quoted);
    }
    Ok(true)
}

fn print_line_info(chip_name: &str, li: &Info, quoted: bool) {
    let lname = if li.name.is_empty() {
        "unnamed".to_string()
    } else if quoted {
        format!("\"{}\"", li.name)
    } else {
        li.name.to_string()
    };
    println!(
        "{} {}\t{:16}\t{}",
        format_chip_name(chip_name),
        li.offset,
        lname,
        stringify_attrs(li, quoted),
    );
}
