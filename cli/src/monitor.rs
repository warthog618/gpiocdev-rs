// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::common::{
    self, ActiveLowOpts, BiasOpts, ChipInfo, EdgeOpts, LineOpts, TimeFmt, UapiOpts,
};
use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use gpiocdev::line::{EdgeEvent, EdgeKind, Offset};
use gpiocdev::request::{Config, Request};
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(aliases(["m", "mon"]))]
pub struct Opts {
    /// The lines to monitor.
    ///
    /// The lines are identified by name or optionally by offset if
    /// the --chip option is specified.
    #[arg(name = "line", required = true)]
    lines: Vec<String>,

    #[command(flatten)]
    line_opts: LineOpts,

    #[command(flatten)]
    active_low_opts: ActiveLowOpts,

    #[command(flatten)]
    bias_opts: BiasOpts,

    #[command(flatten)]
    edge_opts: EdgeOpts,

    /// The debounce period for the monitored lines.
    #[arg(short = 'p', long, name = "period", value_parser = common::parse_duration)]
    debounce_period: Option<Duration>,

    /// Exit after the specified number of events.
    ///
    /// If not specified then monitoring will continue indefinitely.
    #[arg(short, long, name = "num")]
    num_events: Option<u32>,

    /// Specify a custom output format.
    ///
    /// Format specifiers:
    ///   %o   GPIO line offset
    ///   %l   GPIO line name
    ///   %c   GPIO chip path
    ///   %e   numeric edge event type ('1' - rising or '2' - falling)
    ///   %E   edge event type ('rising' or 'falling')
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

    /// Specify the source clock for event timestamps.
    #[arg(short = 'C', long, name = "clock")]
    event_clock: Option<EventClock>,

    /// Format event timestamps as local time.
    #[arg(long, group = "timefmt")]
    localtime: bool,

    /// Format event timestamps as UTC.
    #[arg(long, group = "timefmt")]
    utc: bool,

    #[command(flatten)]
    uapi_opts: UapiOpts,
}

impl Opts {
    // mutate the config to match the configuration
    fn apply(&self, config: &mut Config) {
        if let Some(period) = self.debounce_period {
            config.with_debounce_period(period);
        }
        if let Some(clock) = self.event_clock {
            config.with_event_clock(clock.into());
        } else if self.localtime || self.utc {
            config.with_event_clock(gpiocdev::line::EventClock::Realtime);
        }
        self.active_low_opts.apply(config);
        self.bias_opts.apply(config);
        self.edge_opts.apply(config);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
enum EventClock {
    Monotonic,
    Realtime,
    Hte,
}
impl From<EventClock> for gpiocdev::line::EventClock {
    fn from(c: EventClock) -> Self {
        match c {
            EventClock::Monotonic => gpiocdev::line::EventClock::Monotonic,
            EventClock::Realtime => gpiocdev::line::EventClock::Realtime,
            EventClock::Hte => gpiocdev::line::EventClock::Hte,
        }
    }
}

pub fn cmd(opts: &Opts) -> Result<()> {
    use std::io::Write;

    let timefmt = if opts.localtime {
        TimeFmt::Localtime
    } else if opts.utc || opts.event_clock == Some(EventClock::Realtime) {
        TimeFmt::Utc
    } else {
        TimeFmt::Seconds
    };
    let r = common::resolve_lines(&opts.lines, &opts.line_opts, opts.uapi_opts.abiv)?;
    if r.chips.len() > 1 {
        panic!("presently only support monitoring lines on one chip");
    }
    // needs multi-threading or async - so will come back to this...
    //    for chip in &chips { ...
    let ci = &r.chips[0];
    let mut cfg = Config::default();
    opts.apply(&mut cfg);
    let offsets: Vec<Offset> = r
        .lines
        .values()
        .filter(|co| co.chip_idx == 0)
        .map(|co| co.offset)
        .collect();
    cfg.with_lines(&offsets);
    let req = Request::from_config(cfg)
        .on_chip(&ci.path)
        .with_consumer("gpiocdev-monitor")
        .using_abi_version(common::abi_version_from_opts(opts.uapi_opts.abiv)?)
        .request()
        .context("Failed to request lines.")?;

    let mut count = 0;
    let mut stdout = std::io::stdout();
    for result in req.edge_events() {
        let edge = result.context("Failed to read event.")?;
        print_edge(&edge, &r.chips[0], opts, &timefmt);
        if let Some(limit) = opts.num_events {
            count += 1;
            if count >= limit {
                println!("Exited after {} events.", count);
                break;
            }
        }
        _ = stdout.flush();
    }
    Ok(())
}

fn print_edge(event: &EdgeEvent, ci: &ChipInfo, opts: &Opts, timefmt: &TimeFmt) {
    if let Some(format) = &opts.format {
        return print_edge_formatted(event, format, ci);
    }
    common::print_time(event.timestamp_ns, timefmt);
    print!("\t{}\t", event_kind_name(event.kind));
    if let Some(lname) = ci.named_lines.get(&event.offset) {
        if opts.line_opts.chip.is_some() {
            print!("{} {} ", ci.name, event.offset);
        }
        println!("{}", lname);
    } else {
        println!("{} {}", ci.name, event.offset);
    }
}

fn event_kind_name(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Rising => "rising",
        EdgeKind::Falling => "falling",
    }
}

fn event_kind_num(kind: EdgeKind) -> u8 {
    match kind {
        EdgeKind::Rising => 1,
        EdgeKind::Falling => 2,
    }
}

fn print_edge_formatted(event: &EdgeEvent, format: &str, ci: &ChipInfo) {
    let mut escaped = false;

    for chr in format.chars() {
        if escaped {
            match chr {
                '%' => print!("%"),
                'c' => print!("{}", ci.name),
                'e' => print!("{}", event_kind_num(event.kind)),
                'E' => print!("{}", event_kind_name(event.kind)),
                'l' => common::print_line_name(ci, &event.offset),
                'L' => common::print_time(event.timestamp_ns, &TimeFmt::Localtime),
                'o' => print!("{}", event.offset),
                'S' => common::print_time(event.timestamp_ns, &TimeFmt::Seconds),
                'U' => common::print_time(event.timestamp_ns, &TimeFmt::Utc),
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
