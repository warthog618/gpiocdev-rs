// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::common::{self, ChipInfo, LineOpts, TimeFmt, UapiOpts};
use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use gpiocdev::line::{InfoChangeEvent, InfoChangeKind};
use libc::timespec;
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};
use std::os::unix::prelude::AsRawFd;

#[derive(Debug, Parser)]
#[command(aliases(["n", "watch"]))]
pub struct Opts {
    #[command(flatten)]
    line_opts: LineOpts,

    /// The lines to watch
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

    /// Exit after the specified number of events
    ///
    /// If not specified then watching will continue indefinitely.
    #[arg(short, long, name = "num")]
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
        name = "fmt",
        group = "timefmt",
        verbatim_doc_comment
    )]
    format: Option<String>,

    /// Format event timestamps as local time
    #[arg(long, group = "timefmt")]
    localtime: bool,

    /// Format event timestamps as UTC.
    #[arg(long, group = "timefmt")]
    utc: bool,

    /// Display a banner on successful startup
    #[arg(long)]
    banner: bool,

    /// Don't generate any output
    #[arg(short = 'q', long, group = "timefmt", alias = "silent")]
    quiet: bool,

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
    use std::io::Write;

    let timefmt = if opts.localtime {
        TimeFmt::Localtime
    } else if opts.utc {
        TimeFmt::Utc
    } else {
        TimeFmt::Seconds
    };
    let r = common::resolve_lines(&opts.lines, &opts.line_opts, opts.uapi_opts.abiv)?;
    r.validate(&opts.lines, &opts.line_opts)?;
    let mut poll = Poll::new()?;
    let mut chips = Vec::new();
    for (idx, ci) in r.chips.iter().enumerate() {
        let chip = common::chip_from_path(&ci.path, opts.uapi_opts.abiv)?;

        for offset in r
            .lines
            .values()
            .filter(|co| co.chip_idx == idx)
            .map(|co| co.offset)
        {
            chip.watch_line_info(offset)
                .context(format!("failed to watch line {} on {}", offset, ci.name))?;
        }
        poll.registry().register(
            &mut SourceFd(&chip.as_raw_fd()),
            Token(idx),
            Interest::READABLE,
        )?;
        chips.push(chip);
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
                            while chips[idx].has_line_info_change_event()? {
                                let change = chips[idx].read_line_info_change_event().context(
                                    format!("failed to read event from {}", r.chips[idx].name),
                                )?;
                                if let Some(evtype) = opts.event {
                                    if change.kind != evtype.into() {
                                        continue;
                                    }
                                }
                                if !opts.quiet {
                                    print_change(change, &r.chips[idx], opts, &timefmt);
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
        print!("Watching lines ");

        for l in lines.iter().take(lines.len() - 1) {
            print!("{}, ", l);
        }

        println!("and {}...", lines[lines.len() - 1]);
    } else {
        println!("Watching line {}...", lines[0]);
    }
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
