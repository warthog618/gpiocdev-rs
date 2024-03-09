// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::common::{self, emit_error, format_error, format_time, ChipInfo, EmitOpts, TimeFmt};
use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use gpiocdev::line::{InfoChangeEvent, InfoChangeKind};
use libc::timespec;
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};
#[cfg(feature = "serde")]
use serde_derive::Serialize;
use std::os::unix::prelude::AsRawFd;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(aliases(["n", "watch"]))]
pub struct Opts {
    /// Display a banner on successful startup
    #[arg(long)]
    banner: bool,

    #[command(flatten)]
    line_opts: common::LineOpts,

    /// The lines to watch
    ///
    /// The lines are identified by name or optionally by
    /// offset if the --chip option is specified.
    #[arg(value_name = "line", required = true)]
    lines: Vec<String>,

    /// Specify the events to report.
    ///
    /// Default is all events.
    #[arg(short = 'e', long, value_name = "event")]
    event: Option<EventKind>,

    /// Exit if no events are received for the specified period.
    ///
    /// The period is taken as milliseconds unless otherwise specified.
    #[arg(long, value_name = "period", value_parser = common::parse_duration)]
    idle_timeout: Option<Duration>,

    /// Exit after the specified number of events
    ///
    /// If not specified then watching will continue indefinitely.
    #[arg(short, long, value_name = "num")]
    num_events: Option<u32>,

    /// Specify a custom output format
    ///
    /// Format specifiers:
    ///   %o   GPIO line offset
    ///   %l   GPIO line name
    ///   %c   GPIO chip path
    ///   %e   numeric event type ('1' - requested, '2' - released or '3' - reconfigured)
    ///   %E   event type ('requested', 'released' or 'reconfigured')
    ///   %a   line attributes
    ///   %C   consumer
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

    /// Format event timestamps as UTC.
    #[arg(long, group = "timefmt")]
    utc: bool,

    /// Don't generate any output
    #[arg(short = 'q', long, groups = ["emit", "timefmt"], alias = "silent")]
    quiet: bool,

    #[command(flatten)]
    uapi_opts: common::UapiOpts,

    #[command(flatten)]
    emit: common::EmitOpts,
}

impl Opts {
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

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum EventKind {
    Requested,
    Released,
    Reconfigured,
}
impl From<EventKind> for InfoChangeKind {
    fn from(c: EventKind) -> Self {
        match c {
            EventKind::Requested => InfoChangeKind::Requested,
            EventKind::Released => InfoChangeKind::Released,
            EventKind::Reconfigured => InfoChangeKind::Reconfigured,
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
    let mut chips = Vec::new();
    for (idx, ci) in r.chips.iter().enumerate() {
        let chip = match common::chip_from_path(&ci.path, r.abiv) {
            Ok(c) => c,
            Err(e) => {
                res.push_error(&anyhow!(e).context(format!("failed to open  {}", ci.name)));
                break;
            }
        };

        for offset in r
            .lines
            .values()
            .filter(|co| co.chip_idx == idx)
            .map(|co| co.offset)
        {
            if let Err(e) = chip.watch_line_info(offset) {
                res.push_error(
                    &anyhow!(e).context(format!("failed to watch line {} on {}", offset, ci.name)),
                );
            }
        }
        if let Err(e) = poll.registry().register(
            &mut SourceFd(&chip.as_raw_fd()),
            Token(idx),
            Interest::READABLE,
        ) {
            res.push_error(&anyhow!(e).context(format!("failed register {} with poll", ci.name)));
            return res;
        }
        chips.push(chip);
    }
    if !res.errors.is_empty() {
        return res;
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
                    while chips[idx].has_line_info_change_event().unwrap_or(false) {
                        match chips[idx].read_line_info_change_event() {
                            Ok(change) => {
                                if let Some(evtype) = opts.event {
                                    if change.kind != evtype.into() {
                                        continue;
                                    }
                                }
                                emit_change(change, &r.chips[idx], opts, &timefmt);
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
                        };
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
            eprintln!("{e}");
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
        print!("Watching lines ");

        for l in lines.iter().take(lines.len() - 1) {
            print!("'{}', ", l);
        }

        println!("and '{}'...", lines[lines.len() - 1]);
    } else {
        println!("Watching line '{}'...", lines[0]);
    }
    _ = std::io::stdout().flush();
}

fn emit_change(change: InfoChangeEvent, ci: &ChipInfo, opts: &Opts, timefmt: &TimeFmt) {
    if opts.quiet {
        return;
    }
    let timestamp = format_time(change.timestamp_ns, timefmt);
    let event = Event {
        #[cfg(feature = "json")]
        chip: ci.name.clone(),
        change,
        timestamp,
    };

    #[cfg(feature = "json")]
    if opts.emit.json {
        println!("{}", serde_json::to_string(&event).unwrap());
        return;
    }
    if let Some(format) = &opts.format {
        print_change_formatted(&event.change, format, ci, opts.emit.quoted);
    } else {
        event.print(ci, opts);
    }
}

#[cfg_attr(feature = "serde", derive(Serialize))]
struct Event {
    #[cfg(feature = "json")]
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "String::is_empty"))]
    chip: String,
    #[cfg_attr(feature = "serde", serde(flatten))]
    change: InfoChangeEvent,
    timestamp: String,
}

impl Event {
    fn print(&self, ci: &ChipInfo, opts: &Opts) {
        print!(
            "{}\t{}\t",
            self.timestamp,
            event_kind_name(self.change.kind)
        );

        if let Some(lname) = ci.line_name(&self.change.info.offset) {
            if opts.line_opts.chip.is_some() {
                print!("{} {} ", ci.name, self.change.info.offset);
            }
            if opts.emit.quoted {
                println!("\"{lname}\"");
            } else {
                println!("{lname}");
            }
        } else {
            println!("{} {}", ci.name, self.change.info.offset);
        }
    }
}

fn format_consumer(li: &gpiocdev::line::Info) -> &str {
    if li.used {
        if li.consumer.is_empty() {
            "kernel"
        } else {
            &li.consumer
        }
    } else {
        "unused"
    }
}

fn print_change_formatted(event: &InfoChangeEvent, format: &str, ci: &ChipInfo, quoted: bool) {
    use common::stringify_attrs;

    let mut escaped = false;

    for chr in format.chars() {
        if escaped {
            match chr {
                '%' => print!("%"),
                'a' => print!("{}", stringify_attrs(&event.info, quoted)),
                'c' => print!("{}", ci.name),
                'C' => print!("{}", format_consumer(&event.info)),
                'e' => print!("{}", event_kind_num(event.kind)),
                'E' => print!("{}", event_kind_name(event.kind)),
                'l' => print!("{}", ci.line_name(&event.info.offset).unwrap_or("unnamed")),
                'L' => print!(
                    "{}",
                    format_time(
                        monotonic_to_realtime(event.timestamp_ns),
                        &TimeFmt::Localtime,
                    )
                ),
                'o' => print!("{}", event.info.offset),
                'S' => print!("{}", format_time(event.timestamp_ns, &TimeFmt::Seconds)),
                'U' => print!(
                    "{}",
                    format_time(monotonic_to_realtime(event.timestamp_ns), &TimeFmt::Utc)
                ),
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
