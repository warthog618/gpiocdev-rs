// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    all_chips, chip_from_opts, find_lines, parse_chip_path, string_or_default, stringify_attrs,
    LineOpts, UapiOpts,
};
use anyhow::{Context, Result};
use clap::Parser;
use gpiod::chip::Chip;
use gpiod::line::Offset;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct Opts {
    /// Only get information for the specified lines, identified by name
    /// or optionally by offset if the --chip option is provided.
    /// If not specified then all lines are returned.
    #[clap()]
    lines: Vec<String>,
    /// Restrict operations to lines on this chip.
    #[clap(short, long, parse(from_os_str = parse_chip_path))]
    chip: Option<PathBuf>,
    /// Lines are strictly identified by name, even if that name looks like an offset.
    #[clap(short = 'N', long)]
    by_name: bool,
    #[clap(flatten)]
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
            exhaustive: false,
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
        "{} - {} lines (displaying {})",
        string_or_default(&ci.name.to_string_lossy(), "??"),
        ci.num_lines,
        lines.len(),
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
        "\tline {:>3}:\t{:16}\t{:11} [{}]",
        li.offset,
        string_or_default(&li.name.to_string_lossy(), "unnamed"),
        string_or_default(&li.consumer.to_string_lossy(), "unused"),
        stringify_attrs(&li),
    );
}
