// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::common::{self, emit_error, format_time, ChipInfo, TimeFmt};
use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use gpiocdev::line::{EdgeEvent, EdgeKind, Offset};
use gpiocdev::request::{Config, Request};
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};
use std::os::unix::prelude::AsRawFd;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(aliases(["e", "mon"]))]
pub struct Opts {
    /// The lines to monitor
    ///
    /// The lines are identified by name or optionally by offset if
    /// the --chip option is specified.
    #[arg(value_name = "line", required = true)]
    lines: Vec<String>,

    /// Display a banner on successful startup
    #[arg(long)]
    banner: bool,

    #[command(flatten)]
    line_opts: common::LineOpts,

    #[command(flatten)]
    active_low_opts: common::ActiveLowOpts,

    #[command(flatten)]
    bias_opts: common::BiasOpts,

    #[command(flatten)]
    edge_opts: common::EdgeOpts,

    /// The debounce period for the monitored lines
    ///
    /// The period is taken as milliseconds unless otherwise specified.
    #[cfg(feature = "uapi_v2")]
    #[arg(short = 'p', long, value_name = "period", value_parser = common::parse_duration)]
    debounce_period: Option<Duration>,

    /// Exit if no events are received for the specified period.
    ///
    /// The period is taken as milliseconds unless otherwise specified.
    #[arg(long, value_name = "period", value_parser = common::parse_duration)]
    idle_timeout: Option<Duration>,

    /// Exit after the specified number of events
    ///
    /// If not specified then monitoring will continue indefinitely.
    #[arg(short, long, value_name = "num")]
    num_events: Option<u32>,

    /// Specify the source clock for event timestamps
    #[cfg(feature = "uapi_v2")]
    #[arg(short = 'E', long, value_name = "clock")]
    event_clock: Option<EventClock>,

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
        value_name = "fmt",
        group = "emit",
        verbatim_doc_comment
    )]
    format: Option<String>,

    /// Format event timestamps as local time
    #[arg(long, group = "emit")]
    localtime: bool,

    /// Format event timestamps as UTC
    #[arg(long, group = "emit")]
    utc: bool,

    /// Don't generate any output
    #[arg(short = 'q', long, group = "emit", alias = "silent")]
    quiet: bool,

    /// The consumer label applied to requested lines.
    #[arg(
        short = 'C',
        long,
        value_name = "name",
        default_value = "gpiocdev-edges"
    )]
    consumer: String,

    #[command(flatten)]
    uapi_opts: common::UapiOpts,

    /// Quote line names.
    #[arg(long)]
    quoted: bool,

    #[command(flatten)]
    emit: common::EmitOpts,
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
    let timefmt = time_format_from_opts(opts);
    let abiv = common::actual_abi_version(&opts.uapi_opts)?;
    let r = common::Resolver::resolve_lines(&opts.lines, &opts.line_opts, abiv)?;
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
        bld.using_abi_version(abiv);
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
    let mut events = Events::with_capacity(r.chips.len());
    emit_banner(opts);
    loop {
        match poll.poll(&mut events, opts.idle_timeout) {
            Err(e) => {
                if e.kind() != std::io::ErrorKind::Interrupted {
                    bail!(e);
                }
            }
            Ok(()) => {
                if events.is_empty() {
                    return Ok(true);
                }
                for event in &events {
                    let idx: usize = event.token().into();
                    while reqs[idx].has_edge_event()? {
                        let edge = reqs[idx].read_edge_event().with_context(|| {
                            format!("failed to read event from {}", r.chips[idx].name)
                        })?;
                        emit_edge(&edge, &r.chips[idx], opts, &timefmt);
                        if let Some(limit) = opts.num_events {
                            count += 1;
                            if count >= limit {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn emit_banner(opts: &Opts) {
    if !opts.banner {
        return;
    }
    print_banner(&opts.lines)
}

fn print_banner(lines: &[String]) {
    use std::io::Write;

    if lines.len() > 1 {
        print!("Monitoring lines ");

        for l in lines.iter().take(lines.len() - 1) {
            print!("'{}', ", l);
        }

        println!("and '{}'...", lines[lines.len() - 1]);
    } else {
        println!("Monitoring line '{}'...", lines[0]);
    }
    _ = std::io::stdout().flush();
}

fn emit_edge(event: &EdgeEvent, ci: &ChipInfo, opts: &Opts, timefmt: &TimeFmt) {
    if opts.quiet {
        return;
    }
    print_edge(event, ci, opts, timefmt);
}

fn print_edge(event: &EdgeEvent, ci: &ChipInfo, opts: &Opts, timefmt: &TimeFmt) {
    use std::io::Write;

    if let Some(format) = &opts.format {
        return print_edge_formatted(event, format, ci);
    }
    print!(
        "{}\t{}\t",
        format_time(event.timestamp_ns, timefmt),
        event_kind_name(event.kind)
    );
    if let Some(lname) = ci.named_lines.get(&event.offset) {
        if opts.line_opts.chip.is_some() {
            print!("{} {} ", ci.name, event.offset);
        }
        if opts.quoted {
            println!("\"{}\"", lname);
        } else {
            println!("{}", lname);
        }
    } else {
        println!("{} {}", ci.name, event.offset);
    }
    _ = std::io::stdout().flush();
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
                'l' => print!("{}", ci.line_name(&event.offset).unwrap_or("unnamed")),
                'L' => print!("{}", format_time(event.timestamp_ns, &TimeFmt::Localtime)),
                'o' => print!("{}", event.offset),
                'S' => print!("{}", format_time(event.timestamp_ns, &TimeFmt::Seconds)),
                'U' => print!("{}", format_time(event.timestamp_ns, &TimeFmt::Utc)),
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
