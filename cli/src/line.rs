// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::common::{
    self, actual_abi_version, emit_error, format_chip_name, stringify_attrs, EmitOpts, Resolver,
};
use anyhow::Result;
use clap::Parser;
use gpiocdev::line::Info;

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
    let chips = if let Some(id) = &opts.chip {
        vec![common::chip_lookup_from_id(id)?]
    } else {
        common::all_chip_paths()?
    };
    let r = common::Resolver::resolve_lines_unvalidated(
        &opts.lines,
        &chips,
        actual_abi_version(&opts.uapi_opts)?,
        false,
        opts.strict, // --strict means exhaustive for `line`
        opts.by_name,
        true,
    )?;
    //println!("{:#?}", r);
    emit_line_info(&opts.emit, &opts.lines, &r)?;
    r.validate(&opts.lines, &opts.chip, opts.by_name)?;
    Ok(opts.lines.is_empty()
        || (opts.lines.len() == r.lines.len()) && (opts.lines.len() == r.info.len()))
}

fn emit_line_info(opts: &EmitOpts, lines: &[String], r: &Resolver) -> Result<()> {
    if lines.is_empty() {
        for idx in 0..r.chips.len() {
            print_chip_lines(r, idx, opts);
        }
    } else {
        for info in &r.info {
            print_line_info(&r.chips[info.chip].name, &info.info, opts.quoted)
        }
    }
    Ok(())
}

fn print_chip_lines(r: &Resolver, idx: usize, opts: &EmitOpts) {
    let c = &r.chips[idx];
    println!("{} - {} lines:", format_chip_name(&c.name), c.num_lines);
    for info in &r.info {
        if info.chip != idx {
            continue;
        }
        let li = &info.info;
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
            stringify_attrs(li, opts.quoted),
        );
    }
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
