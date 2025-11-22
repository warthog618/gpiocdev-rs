// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::common::{
    self, emit_error, format_chip_name, stringify_attrs, EmitOpts, LineOpts, Resolver,
};
use clap::Parser;
use gpiocdev::line::Info;
#[cfg(feature = "serde")]
use serde_derive::Serialize;

#[derive(Debug, Default, Parser)]
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
    let line_opts = LineOpts {
        chip: opts.chip.clone(),
        strict: false, // to continue on multi-match
        by_name: opts.by_name,
    };
    let res = Cmd {
        opts,
        r: common::Resolver::resolve_lines_with_info(
            &opts.lines,
            &line_opts,
            &opts.uapi_opts,
            opts.strict, // --strict means exhaustive for `line`
            true,
        ),
    };
    res.emit();
    res.is_success()
}

#[cfg_attr(feature = "serde", derive(Serialize))]
struct Cmd<'a> {
    #[cfg_attr(feature = "serde", serde(skip))]
    opts: &'a Opts,
    #[cfg_attr(feature = "serde", serde(skip))]
    r: Resolver,
}

impl Cmd<'_> {
    fn is_success(&self) -> bool {
        self.r.errors.is_empty()
            && (self.opts.lines.is_empty()
                || (self.opts.lines.len() == self.r.lines.len())
                    && (self.opts.lines.len() == self.r.info.len()))
    }

    fn emit(&self) {
        #[cfg(feature = "json")]
        if self.opts.emit.json {
            self.emit_json();
            return;
        }
        self.print();
    }

    #[cfg(feature = "json")]
    fn emit_json(&self) {
        let mut res = CmdResult {
            ..Default::default()
        };
        for i in &self.r.info {
            res.lines.push(LineInfo {
                chip: &self.r.chips[i.chip].name,
                info: &i.info,
            });
        }
        for e in &self.r.errors {
            res.errors.push(common::format_error(&self.opts.emit, e))
        }
        println!(
            "{}",
            serde_json::to_string(&res).expect("serialize should succeed")
        );
    }

    fn print(&self) {
        if self.opts.lines.is_empty() {
            for idx in 0..self.r.chips.len() {
                print_chip_lines(&self.r, idx, &self.opts.emit);
            }
        } else {
            for info in &self.r.info {
                print_line_info(
                    &self.r.chips[info.chip].name,
                    &info.info,
                    self.opts.emit.quoted,
                )
            }
        }
        for e in &self.r.errors {
            emit_error(&self.opts.emit, e);
        }
    }
}

#[cfg(feature = "serde")]
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(Serialize))]
struct CmdResult<'a> {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    lines: Vec<LineInfo<'a>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    errors: Vec<String>,
}
#[cfg(feature = "serde")]
#[cfg_attr(feature = "serde", derive(Serialize))]
struct LineInfo<'a> {
    chip: &'a str,
    #[cfg_attr(feature = "serde", serde(flatten))]
    info: &'a Info,
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
