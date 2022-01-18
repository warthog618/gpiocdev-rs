// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{all_chips, chip_from_opts, parse_chip_path, string_or_default, UapiOpts};
use anyhow::{Context, Result};
use gpiod::chip::Chip;
use gpiod::line::{Bias, Direction, Drive, EdgeDetection, EventClock, Offset};
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
        "\tline {:>3}:{:>12} {:>11} [{}]",
        li.offset,
        string_or_default(&li.name.to_string_lossy(), "unnamed"),
        string_or_default(&li.consumer.to_string_lossy(), "unused"),
        stringify_attrs(&li),
    );
}

fn stringify_attrs(li: &gpiod::line::Info) -> String {
    let mut attrs = Vec::new();
    match li.direction {
        gpiod::line::Direction::Input => attrs.push("input"),
        Direction::Output => attrs.push("output"),
    }
    if li.used {
        attrs.push("used");
    }
    if li.active_low {
        attrs.push("active-low");
    }
    match li.drive {
        None => (),
        Some(Drive::PushPull) => (),
        Some(Drive::OpenDrain) => attrs.push("open-drain"),
        Some(Drive::OpenSource) => attrs.push("open-source"),
    }
    match li.bias {
        None => (),
        Some(Bias::PullUp) => attrs.push("pull-up"),
        Some(Bias::PullDown) => attrs.push("pull-down"),
        Some(Bias::Disabled) => attrs.push("bias-disabled"),
    }
    match li.edge_detection {
        None => (),
        Some(EdgeDetection::RisingEdge) => attrs.push("rising-edge"),
        Some(EdgeDetection::FallingEdge) => attrs.push("falling-edge"),
        Some(EdgeDetection::BothEdges) => attrs.push("both-edges"),
    }
    match li.event_clock {
        None => (),                        // Not present for v1.
        Some(EventClock::Monotonic) => (), // default for ABI v2
        Some(EventClock::Realtime) => attrs.push("event-clock-realtime"),
    }
    let db;
    if li.debounce_period.is_some() {
        db = format!("debounce_period={:?}", li.debounce_period.unwrap());
        attrs.push(&db);
    }
    attrs.join(", ")
}
