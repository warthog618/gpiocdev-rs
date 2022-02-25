// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    all_chips, chip_from_opts, parse_chip_path, string_or_default, stringify_attrs, UapiOpts,
};
use anyhow::{Context, Result};
use gpiod::chip::Chip;
use gpiod::line::Offset;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Opts {
    /// The chip to interrogate.  If not specified then all chips are searched.
    #[structopt(parse(from_os_str = parse_chip_path))]
    chip: Option<PathBuf>,
    /// The offsets of the lines to get.  In not specified then all lines are returned.
    #[structopt(short, long, requires("chip"))]
    lines: Vec<Offset>,
    #[structopt(flatten)]
    uapi_opts: UapiOpts,
}

pub fn cmd(opts: &Opts) -> Result<()> {
    if let Some(p) = &opts.chip {
        let mut c = chip_from_opts(p, opts.uapi_opts.abiv)?;
        if opts.lines.is_empty() {
            print_chip_all_line_info(&mut c)?;
        } else {
            return print_chip_line_info(&mut c, &opts.lines);
        }
    } else {
        for p in all_chips()? {
            let mut c = chip_from_opts(&p, opts.uapi_opts.abiv)?;
            print_chip_all_line_info(&mut c)?;
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
        string_or_default(&ci.name.to_string_lossy(), "??"),
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
        print_line_info(li);
    }
    Ok(())
}

fn print_chip_line_info(chip: &mut Chip, lines: &[Offset]) -> Result<()> {
    let ci = chip
        .info()
        .with_context(|| format!("Failed to read info from chip {:?}.", chip.path()))?;
    println!(
        "{} - {} of {} lines",
        string_or_default(&ci.name.to_string_lossy(), "??"),
        lines.len(),
        ci.num_lines
    );
    for &offset in lines {
        let li = chip.line_info(offset).with_context(|| {
            format!(
                "Failed to read line {} info from chip {:?}.",
                offset,
                chip.path()
            )
        })?;
        print_line_info(li);
    }
    Ok(())
}

fn print_line_info(li: gpiod::line::Info) {
    println!(
        "\tline {:>3}: {:>16} {:>11} [{}]",
        li.offset,
        string_or_default(&li.name.to_string_lossy(), "unnamed"),
        string_or_default(&li.consumer.to_string_lossy(), "unused"),
        stringify_attrs(&li),
    );
}
