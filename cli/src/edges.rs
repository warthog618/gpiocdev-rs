// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::common::{
    self, ActiveLowOpts, BiasOpts, ChipInfo, EdgeOpts, LineOpts, TimeFmt, UapiOpts,
};
use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use gpiocdev::line::{EdgeEvent, EdgeKind, Offset};
use gpiocdev::request::{Config, Request};
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};
use std::os::unix::prelude::AsRawFd;
#[cfg(feature = "uapi_v2")]
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(aliases(["e", "mon"]))]
pub struct Opts {
    /// The lines to monitor
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

    /// The debounce period for the monitored lines
    ///
    /// The period is taken as milliseconds unless otherwise specified.
    #[cfg(feature = "uapi_v2")]
    #[arg(short = 'p', long, name = "period", value_parser = common::parse_duration)]
    debounce_period: Option<Duration>,

    /// Exit after the specified number of events
    ///
    /// If not specified then monitoring will continue indefinitely.
    #[arg(short, long, name = "num")]
    num_events: Option<u32>,

    /// Specify a custom output format
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

    /// Specify the source clock for event timestamps
    #[cfg(feature = "uapi_v2")]
    #[arg(short = 'C', long, name = "clock")]
    event_clock: Option<EventClock>,

    /// Format event timestamps as local time
    #[arg(long, group = "timefmt")]
    localtime: bool,

    /// Format event timestamps as UTC
    #[arg(long, group = "timefmt")]
    utc: bool,

    /// Display a banner on successful startup
    #[arg(long)]
    banner: bool,

    /// Don't generate any output
    #[arg(short = 'q', long, group = "timefmt", alias = "silent")]
    quiet: bool,

    /// The consumer label applied to requested lines.
    #[arg(long, name = "consumer", default_value = "gpiocdev-edges")]
    consumer: String,

    #[command(flatten)]
    uapi_opts: UapiOpts,
}

impl Opts {
    // mutate the config to match the configuration
    fn apply(&self, config: &mut Config) {
        #[cfg(feature = "uapi_v2")]
        {
            if let Some(period) = self.debounce_period {
                config.with_debounce_period(period);
            }
            if let Some(clock) = self.event_clock {
                config.with_event_clock(clock.into());
            } else if self.localtime || self.utc {
                config.with_event_clock(gpiocdev::line::EventClock::Realtime);
            }
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

#[cfg(feature = "uapi_v2")]
fn time_format_from_opts(opts: &Opts) -> TimeFmt {
    if opts.localtime {
        TimeFmt::Localtime
    } else if opts.utc || opts.event_clock == Some(EventClock::Realtime) {
        TimeFmt::Utc
    } else {
        TimeFmt::Seconds
    }
}
#[cfg(not(feature = "uapi_v2"))]
fn time_format_from_opts(opts: &Opts) -> TimeFmt {
    if opts.localtime {
        TimeFmt::Localtime
    } else if opts.utc {
        TimeFmt::Utc
    } else {
        TimeFmt::Seconds
    }
}

pub fn cmd(opts: &Opts) -> Result<()> {
    use std::io::Write;

    let timefmt = time_format_from_opts(opts);
    let r = common::resolve_lines(&opts.lines, &opts.line_opts, opts.uapi_opts.abiv)?;
    r.validate(&opts.lines, &opts.line_opts)?;
    let mut poll = Poll::new()?;
    let mut reqs = Vec::new();
    for (idx, ci) in r.chips.iter().enumerate() {
        let mut cfg = Config::default();
        opts.apply(&mut cfg);
        let offsets: Vec<Offset> = r
            .lines
            .values()
            .filter(|co| co.chip_idx == idx)
            .map(|co| co.offset)
            .collect();
        cfg.with_lines(&offsets);
        let mut bld = Request::from_config(cfg);
        bld.on_chip(&ci.path).with_consumer(&opts.consumer);
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        bld.using_abi_version(common::abi_version_from_opts(opts.uapi_opts.abiv)?);
        let req = bld
            .request()
            .with_context(|| format!("failed to request lines {:?} from {}", offsets, ci.name))?;
        poll.registry().register(
            &mut SourceFd(&req.as_raw_fd()),
            Token(idx),
            Interest::READABLE,
        )?;
        reqs.push(req);
    }
    let mut count = 0;
    let mut stdout = std::io::stdout();
    let mut events = Events::with_capacity(r.chips.len());
    if opts.banner {
        print_banner(&opts.lines);
        _ = stdout.flush();
    }
    loop {
        match poll.poll(&mut events, None) {
            Err(e) => {
                if e.kind() != std::io::ErrorKind::Interrupted {
                    bail!(e);
                }
            }
            Ok(()) => {
                for event in &events {
                    match event.token() {
                        Token(idx) => {
                            while reqs[idx].has_edge_event()? {
                                if !opts.quiet {
                                    let edge = reqs[idx].read_edge_event().with_context(|| {
                                        format!("failed to read event from {}", r.chips[idx].name)
                                    })?;
                                    print_edge(&edge, &r.chips[idx], opts, &timefmt);
                                    _ = stdout.flush();
                                }
                                if let Some(limit) = opts.num_events {
                                    count += 1;
                                    if count >= limit {
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn print_banner(lines: &[String]) {
    if lines.len() > 1 {
        print!("Monitoring lines ");

        for l in lines.iter().take(lines.len() - 1) {
            print!("{}, ", l);
        }

        println!("and {}...", lines[lines.len() - 1]);
    } else {
        println!("Monitoring line {}...", lines[0]);
    }
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
