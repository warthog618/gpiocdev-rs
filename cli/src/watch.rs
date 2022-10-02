// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::common::{self, ChipInfo, LineOpts, TimeFmt, UapiOpts};
use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use gpiocdev::line::{InfoChangeEvent, InfoChangeKind};
use libc::timespec;

#[derive(Debug, Parser)]
#[command(alias("w"))]
pub struct Opts {
    #[command(flatten)]
    line_opts: LineOpts,

    /// The lines to watch.
    ///
    /// The lines are identified by name or optionally by
    /// offset if the --chip option is specified.
    #[arg(name = "line", required = true)]
    lines: Vec<String>,

    /// Specify the events to report.
    ///
    /// Default is all events.
    #[arg(short = 'e', long, name = "event")]
    event: Option<Event>,

    /// Specify a custom output format.
    ///
    /// Format specifiers:
    ///   %o   GPIO line offset
    ///   %l   GPIO line name
    ///   %c   GPIO chip path
    ///   %e   numeric event type ('1' - requested, '2' - released or '3' - reconfigured
    ///   %E   event type ('requested', 'released' or 'reconfigured')
    ///   %a   line attributes
    ///   %C   consumer
    ///   %S   event timestamp as seconds
    ///   %U   event timestamp as UTC
    ///   %L   event timestamp as local time
    #[arg(
        short = 'F',
        long,
        name = "fmt",
        group = "timefmt",
        verbatim_doc_comment
    )]
    format: Option<String>,

    /// Format event timestamps as local time.
    #[arg(long, group = "timefmt")]
    localtime: bool,

    /// Format event timestamps as UTC.
    #[arg(long, group = "timefmt")]
    utc: bool,

    #[command(flatten)]
    uapi_opts: UapiOpts,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Event {
    Requested,
    Released,
    Reconfigured,
}
impl From<Event> for InfoChangeKind {
    fn from(c: Event) -> Self {
        match c {
            Event::Requested => InfoChangeKind::Requested,
            Event::Released => InfoChangeKind::Released,
            Event::Reconfigured => InfoChangeKind::Reconfigured,
        }
    }
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let timefmt = if opts.localtime {
        TimeFmt::Localtime
    } else if opts.utc {
        TimeFmt::Utc
    } else {
        TimeFmt::Seconds
    };
    let r = common::resolve_lines(&opts.lines, &opts.line_opts, opts.uapi_opts.abiv)?;
    if r.chips.len() > 1 {
        panic!("presently only support watching lines on one chip");
    }
    // needs multi-threading or async - so will come back to this...
    //    for chip in &chips { ...
    let chip = common::chip_from_opts(&r.chips[0].path, opts.uapi_opts.abiv)?;

    for offset in r
        .lines
        .values()
        .filter(|co| co.chip_idx == 0)
        .map(|co| co.offset)
    {
        chip.watch_line_info(offset)
            .context("Failed to watch lines.")?;
    }

    for result in chip.info_change_events() {
        let change = result.context("Failed to read event.")?;
        if let Some(evtype) = opts.event {
            if change.kind != evtype.into() {
                continue;
            }
        }
        print_change(change, &r.chips[0], opts, &timefmt);
    }
    Ok(())
}

fn print_change(event: InfoChangeEvent, ci: &ChipInfo, opts: &Opts, timefmt: &TimeFmt) {
    if let Some(format) = &opts.format {
        return print_change_formatted(&event, format, ci);
    }
    let evtime = if opts.utc || opts.localtime {
        monotonic_to_realtime(event.timestamp_ns)
    } else {
        event.timestamp_ns
    };
    common::print_time(evtime, timefmt);
    print!("\t{}\t", event_kind_name(event.kind));

    if let Some(lname) = ci.named_lines.get(&event.info.offset) {
        if opts.line_opts.chip.is_some() {
            print!("{} {} ", ci.name, event.info.offset);
        }
        println!("{}", lname);
    } else {
        println!("{} {}", ci.name, event.info.offset);
    }
}

fn print_change_formatted(event: &InfoChangeEvent, format: &str, ci: &ChipInfo) {
    let mut escaped = false;

    for chr in format.chars() {
        if escaped {
            match chr {
                '%' => print!("%"),
                'a' => print!("{}", common::stringify_attrs(&event.info)),
                'c' => print!("{}", ci.name),
                'C' => common::print_consumer(&event.info),
                'e' => print!("{}", event_kind_num(event.kind)),
                'E' => print!("{}", event_kind_name(event.kind)),
                'l' => common::print_line_name(ci, &event.info.offset),
                'L' => common::print_time(
                    monotonic_to_realtime(event.timestamp_ns),
                    &TimeFmt::Localtime,
                ),
                'o' => print!("{}", event.info.offset),
                'S' => common::print_time(event.timestamp_ns, &TimeFmt::Seconds),
                'U' => common::print_time(monotonic_to_realtime(event.timestamp_ns), &TimeFmt::Utc),
                x => print!("%{}", x),
            }
            escaped = false;
        } else if chr == '%' {
            escaped = true;
        } else {
            print!("{}", chr);
        }
    }
    if escaped {
        print!("%");
    }
    println!();
}

fn event_kind_name(kind: InfoChangeKind) -> &'static str {
    match kind {
        InfoChangeKind::Requested => "requested",
        InfoChangeKind::Released => "released",
        InfoChangeKind::Reconfigured => "reconfigured",
    }
}

fn event_kind_num(kind: InfoChangeKind) -> u8 {
    match kind {
        InfoChangeKind::Requested => 1,
        InfoChangeKind::Released => 2,
        InfoChangeKind::Reconfigured => 3,
    }
}

fn monotonic_to_realtime(timestamp: u64) -> u64 {
    use libc::{CLOCK_MONOTONIC, CLOCK_REALTIME};

    // SAFETY: tspec is self-contained
    unsafe {
        let mut tspec = timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        libc::clock_gettime(CLOCK_REALTIME, std::ptr::addr_of_mut!(tspec));
        let before = timespec_to_ns(&tspec);
        libc::clock_gettime(CLOCK_MONOTONIC, std::ptr::addr_of_mut!(tspec));
        let mono = timespec_to_ns(&tspec);
        libc::clock_gettime(CLOCK_REALTIME, std::ptr::addr_of_mut!(tspec));
        let after = timespec_to_ns(&tspec);
        timestamp + after / 2 - mono + before / 2
    }
}

fn timespec_to_ns(ts: &libc::timespec) -> u64 {
    (ts.tv_sec as u64) * 1000000000 + (ts.tv_nsec as u64)
}
