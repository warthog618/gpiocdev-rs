// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::common::{self, emit_error, format_time, ChipInfo, TimeFmt};
use anyhow::{anyhow, Context, Result};
use clap::{Parser, ValueEnum};
use gpiocdev::line::{InfoChangeEvent, InfoChangeKind};
use libc::timespec;
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};
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
    event: Option<Event>,

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
        group = "emit",
        verbatim_doc_comment
    )]
    format: Option<String>,

    /// Format event timestamps as local time
    #[arg(long, group = "emit")]
    localtime: bool,

    /// Format event timestamps as UTC.
    #[arg(long, group = "emit")]
    utc: bool,

    /// Don't generate any output
    #[arg(short = 'q', long, group = "emit", alias = "silent")]
    quiet: bool,

    #[command(flatten)]
    uapi_opts: common::UapiOpts,

    #[command(flatten)]
    emit: common::EmitOpts,
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
    let timefmt = if opts.localtime {
        TimeFmt::Localtime
    } else if opts.utc {
        TimeFmt::Utc
    } else {
        TimeFmt::Seconds
    };
    let abiv = common::actual_abi_version(&opts.uapi_opts)?;
    let r = common::Resolver::resolve_lines(&opts.lines, &opts.line_opts, abiv)?;
    let mut poll = Poll::new()?;
    let mut chips = Vec::new();
    for (idx, ci) in r.chips.iter().enumerate() {
        let chip = common::chip_from_path(&ci.path, abiv)?;

        for offset in r
            .lines
            .values()
            .filter(|co| co.chip_idx == idx)
            .map(|co| co.offset)
        {
            chip.watch_line_info(offset)
                .with_context(|| format!("failed to watch line {} on {}", offset, ci.name))?;
        }
        poll.registry().register(
            &mut SourceFd(&chip.as_raw_fd()),
            Token(idx),
            Interest::READABLE,
        )?;
        chips.push(chip);
    }
    let mut count = 0;
    let mut events = Events::with_capacity(r.chips.len());
    emit_banner(opts);
    loop {
        match poll.poll(&mut events, opts.idle_timeout) {
            Err(e) => {
                if e.kind() != std::io::ErrorKind::Interrupted {
                    emit_error(&opts.emit, &anyhow!(e));
                    return Ok(false);
                }
            }
            Ok(()) => {
                if events.is_empty() {
                    return Ok(true);
                }
                for event in &events {
                    let idx: usize = event.token().into();
                    while chips[idx].has_line_info_change_event()? {
                        let change =
                            chips[idx].read_line_info_change_event().with_context(|| {
                                format!("failed to read event from {}", r.chips[idx].name)
                            })?;
                        if let Some(evtype) = opts.event {
                            if change.kind != evtype.into() {
                                continue;
                            }
                        }
                        emit_change(change, &r.chips[idx], opts, &timefmt);
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

fn emit_change(event: InfoChangeEvent, ci: &ChipInfo, opts: &Opts, timefmt: &TimeFmt) {
    if opts.quiet {
        return;
    }
    print_change(event, ci, opts, timefmt);
}

fn print_change(event: InfoChangeEvent, ci: &ChipInfo, opts: &Opts, timefmt: &TimeFmt) {
    use std::io::Write;

    if let Some(format) = &opts.format {
        return print_change_formatted(&event, format, ci, opts.emit.quoted);
    }
    let evtime = if opts.utc || opts.localtime {
        monotonic_to_realtime(event.timestamp_ns)
    } else {
        event.timestamp_ns
    };
    print!(
        "{}\t{}\t",
        format_time(evtime, timefmt),
        event_kind_name(event.kind)
    );

    if let Some(lname) = ci.line_name(&event.info.offset) {
        if opts.line_opts.chip.is_some() {
            print!("{} {} ", ci.name, event.info.offset);
        }
        if opts.emit.quoted {
            println!("\"{}\"", lname);
        } else {
            println!("{}", lname);
        }
    } else {
        println!("{} {}", ci.name, event.info.offset);
    }
    _ = std::io::stdout().flush();
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
