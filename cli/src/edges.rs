// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::common::{self, emit_error, format_error, format_time, ChipInfo, EmitOpts, TimeFmt};
use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use gpiocdev::line::{EdgeEvent, EdgeKind, Offset};
use gpiocdev::request::{Config, Request};
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};
#[cfg(feature = "serde")]
use serde_derive::Serialize;
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
        groups = ["emit", "timefmt"],
        verbatim_doc_comment
    )]
    format: Option<String>,

    /// Format event timestamps as local time
    #[arg(long, group = "timefmt")]
    localtime: bool,

    /// Format event timestamps as UTC
    #[arg(long, group = "timefmt")]
    utc: bool,

    /// Don't generate any output
    #[arg(short = 'q', long, groups = ["emit", "timefmt"], alias = "silent")]
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
                config.with_event_clock(Some(clock.into()));
            } else if self.localtime || self.utc {
                config.with_event_clock(gpiocdev::line::EventClock::Realtime);
            }
        }
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        if self.event_clock.is_none() {
            // uapi v1 can't select clock, so we can't force Realtime for
            // utc or localtime - we just have to hope the kernel uses Realtime.
            if let Some(super::common::AbiVersionFlags::V1) = self.uapi_opts.abi_version {
                config.with_event_clock(None);
            }
        }
        self.active_low_opts.apply(config);
        self.bias_opts.apply(config);
        self.edge_opts.apply(config);
    }

    #[cfg(feature = "uapi_v2")]
    fn timefmt(&self) -> TimeFmt {
        if self.localtime {
            TimeFmt::Localtime
        } else if self.utc || self.event_clock == Some(EventClock::Realtime) {
            TimeFmt::Utc
        } else {
            TimeFmt::Seconds
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn timefmt(&self) -> TimeFmt {
        if self.localtime {
            TimeFmt::Localtime
        } else if self.utc {
            TimeFmt::Utc
        } else {
            TimeFmt::Seconds
        }
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

pub fn cmd(opts: &Opts) -> bool {
    let res = do_cmd(opts);
    res.emit();
    res.errors.is_empty()
}

fn do_cmd(opts: &Opts) -> CmdResults {
    use std::io::Write;

    let mut res = CmdResults {
        opts: opts.emit,
        ..Default::default()
    };
    let r = common::Resolver::resolve_lines(&opts.lines, &opts.line_opts, &opts.uapi_opts);
    if !r.errors.is_empty() {
        for e in r.errors {
            res.push_error(&e);
        }
        return res;
    }
    let mut poll = match Poll::new() {
        Ok(p) => p,
        Err(e) => {
            res.push_error(&anyhow!(e).context("failed to create poll"));
            return res;
        }
    };

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
        bld.using_abi_version(r.abiv);
        match bld.request() {
            Ok(req) => {
                if let Err(e) = poll.registry().register(
                    &mut SourceFd(&req.as_raw_fd()),
                    Token(idx),
                    Interest::READABLE,
                ) {
                    res.push_error(&anyhow!(e).context(format!(
                        "failed register {:?} from {} with poll",
                        offsets, ci.name
                    )));
                    return res;
                }
                reqs.push(req);
            }
            Err(e) => {
                res.push_error(&anyhow!(e).context(format!(
                    "failed to request lines {:?} from {}",
                    offsets, ci.name
                )));
                return res;
            }
        }
    }
    let mut count = 0;
    let mut events = Events::with_capacity(r.chips.len());
    let timefmt = opts.timefmt();
    emit_banner(opts);
    loop {
        match poll.poll(&mut events, opts.idle_timeout) {
            Err(e) => {
                if e.kind() != std::io::ErrorKind::Interrupted {
                    res.push_error(&anyhow!(e));
                    return res;
                }
            }
            Ok(()) => {
                if events.is_empty() {
                    return res;
                }
                for event in &events {
                    let idx: usize = event.token().into();
                    while reqs[idx].has_edge_event().unwrap_or(false) {
                        match reqs[idx].read_edge_event() {
                            Ok(edge) => {
                                emit_edge(edge, &r.chips[idx], opts, &timefmt);
                                if let Some(limit) = opts.num_events {
                                    count += 1;
                                    if count >= limit {
                                        return res;
                                    }
                                }
                            }
                            Err(e) => {
                                emit_error(
                                    &opts.emit,
                                    &anyhow!(e).context(format!(
                                        "failed to read event from {}",
                                        r.chips[idx].name
                                    )),
                                );
                            }
                        }
                    }
                }
                _ = std::io::stdout().flush();
            }
        }
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(Serialize))]
struct CmdResults {
    #[cfg_attr(feature = "serde", serde(skip))]
    opts: EmitOpts,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    errors: Vec<String>,
}
impl CmdResults {
    fn emit(&self) {
        #[cfg(feature = "json")]
        if self.opts.json {
            println!("{}", serde_json::to_string(self).unwrap());
            return;
        }
        for e in &self.errors {
            eprintln!("{}", e);
        }
    }

    fn push_error(&mut self, e: &anyhow::Error) {
        self.errors.push(format_error(&self.opts, e))
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

fn emit_edge(edge: EdgeEvent, ci: &ChipInfo, opts: &Opts, timefmt: &TimeFmt) {
    if opts.quiet {
        return;
    }
    let timestamp = format_time(edge.timestamp_ns, timefmt);
    let line_name = ci.line_name(&edge.offset).map(|x| x.into());
    let event = Event {
        #[cfg(feature = "json")]
        chip: ci.name.clone(),
        name: line_name,
        edge,
        timestamp,
    };

    #[cfg(feature = "json")]
    if opts.emit.json {
        println!("{}", serde_json::to_string(&event).unwrap());
        return;
    }
    if let Some(format) = &opts.format {
        return print_edge_formatted(&event.edge, format, ci);
    }
    event.print(ci, opts);
}

#[cfg_attr(feature = "serde", derive(Serialize))]
struct Event {
    #[cfg(feature = "json")]
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "String::is_empty"))]
    chip: String,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    name: Option<String>,
    #[cfg_attr(feature = "serde", serde(flatten))]
    edge: EdgeEvent,
    timestamp: String,
}

impl Event {
    fn print(&self, ci: &ChipInfo, opts: &Opts) {
        print!("{}\t{}\t", self.timestamp, event_kind_name(self.edge.kind));
        if let Some(lname) = &self.name {
            if opts.line_opts.chip.is_some() {
                print!("{} {} ", ci.name, self.edge.offset);
            }
            if opts.emit.quoted {
                println!("\"{}\"", lname);
            } else {
                println!("{}", lname);
            }
        } else {
            println!("{} {}", ci.name, self.edge.offset);
        }
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
