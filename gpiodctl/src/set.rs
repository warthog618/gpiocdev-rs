// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    abi_version_from_opts, find_lines, parse_duration, ActiveLowOpts, BiasOpts, DriveOpts,
    LineOpts, ParseDurationError, UapiOpts,
};
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use colored::*;
use gpiod::line::{Offset, Value, Values};
use gpiod::request::{Config, Request};
use rustyline::completion::{Completer, Pair};
use rustyline::config::CompletionType;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};
use std::cmp::max;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::str::{FromStr, SplitWhitespace};
use std::thread::sleep;
use std::time::Duration;
#[derive(Debug, Parser)]
pub struct Opts {
    /// The line values.
    ///
    /// The values are specified in name=value format or optionally in offset=value
    /// format if the --chip option is provided.
    ///
    /// Values may be inactive/off/false/0 or active/on/true/1.
    /// e.g.
    ///     GPIO17=on GPIO22=inactive
    ///     --chip gpiochip0 17=1 22=0
    #[clap(name = "line=value", required = true, parse(try_from_str = parse_key_val), verbatim_doc_comment)]
    line_values: Vec<(String, LineValue)>,
    #[clap(flatten)]
    line_opts: LineOpts,
    #[clap(flatten)]
    active_low_opts: ActiveLowOpts,
    #[clap(flatten)]
    bias_opts: BiasOpts,
    #[clap(flatten)]
    drive_opts: DriveOpts,
    /// Set the lines then wait for additional set commands for the requested lines.
    ///
    /// Use the "help" command at the interactive prompt to get help for
    /// the supported commands.
    #[clap(short, long, group = "mode")]
    interactive: bool,
    /// The minimum time period to hold lines at the requested values.
    #[clap(short = 'p', long, name = "period", parse(try_from_str = parse_duration))]
    hold_period: Option<Duration>,
    /// Toggle the lines after the specified time periods.
    ///
    /// The time periods are a comma separated list.
    /// The lines are toggled after the period elapses, so the initial time period
    /// applies to the initial line value.
    /// If the final period is not zero, then the sequence repeats.
    ///
    ///  e.g.
    ///      -t 10ms
    ///      -t 100us,200us,100us,150us
    ///      -t 1s,2s,1s,0s
    ///
    /// The first two examples repeat, the third exits after 4s.
    ///
    /// A 0s period elsewhere in the sequence is toggled as fast as possible,
    /// allowing for any specified --hold-period.
    #[clap(short = 't', long, name="periods", parse(try_from_str = parse_time_sequence), group = "mode", verbatim_doc_comment)]
    toggle: Option<TimeSequence>,
    #[clap(flatten)]
    uapi_opts: UapiOpts,
}

impl Opts {
    // mutate the config to match the configuration
    fn apply<'b>(&self, config: &'b mut Config) -> &'b mut Config {
        self.active_low_opts.apply(config);
        self.bias_opts.apply(config);
        self.drive_opts.apply(config)
    }
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let mut setter = Setter {
        hold_period: opts.hold_period,
        ..Default::default()
    };
    setter.request(opts)?;
    if let Some(ts) = &opts.toggle {
        return setter.toggle(ts);
    }
    setter.hold();
    if opts.interactive {
        setter.interact(opts)?;
    }
    Ok(())
}

#[derive(Default)]
struct Setter {
    // Map from command line name to top level line details.
    lines: HashMap<String, Line>,
    // The list of chips containing requested lines,
    // in the same order as the lines occur  on the command line.
    chips: Vec<PathBuf>,
    // The request on each chip
    requests: Vec<Request>,
    // The minimum period to hold set values before applying the subsequent set.
    hold_period: Option<Duration>,
    // Flag indicating if last operation resulted in a hold
    last_held: bool,
}

impl Setter {
    fn request(&mut self, opts: &Opts) -> Result<()> {
        let line_names: Vec<String> = opts
            .line_values
            .iter()
            .map(|(l, _v)| l.to_owned())
            .collect();
        let (lines, chips) = find_lines(&line_names, &opts.line_opts, opts.uapi_opts.abiv)?;
        self.chips = chips;

        // find set of lines for each chip
        for (line_id, v) in &opts.line_values {
            let co = lines.get(line_id).unwrap();
            self.lines.insert(
                line_id.to_owned(),
                Line {
                    chip: co.chip.to_owned(),
                    offset: co.offset,
                    value: v.0,
                    dirty: false,
                },
            );
        }

        // request the lines
        for chip in &self.chips {
            let mut cfg = Config::default();
            opts.apply(&mut cfg);
            for line in self.lines.values() {
                if &line.chip == chip {
                    cfg.with_line(line.offset).as_output(line.value);
                }
            }
            let req = Request::from_config(cfg)
                .on_chip(&chip)
                .with_consumer("gpiodctl-set")
                .using_abi_version(abi_version_from_opts(opts.uapi_opts.abiv)?)
                .request()
                .with_context(|| format!("Failed to request and set lines on chip {:?}.", chip))?;
            self.requests.push(req);
        }
        Ok(())
    }

    fn interact(&mut self, opts: &Opts) -> Result<()> {
        let helper = InteractiveHelper {
            line_names: opts
                .line_values
                .iter()
                .map(|(l, _v)| l.to_owned())
                .collect(),
        };
        let config = rustyline::Config::builder()
            .completion_type(CompletionType::List)
            .auto_add_history(true)
            .max_history_size(20)
            .history_ignore_space(true)
            .build();
        let mut rl = Editor::with_config(config);
        rl.set_helper(Some(helper));
        loop {
            let readline = rl.readline("gpiodctl-set> ");
            match readline {
                Ok(line) => {
                    let mut words = line.trim().split_ascii_whitespace();
                    if let Err(err) = match words.next() {
                        None => continue,
                        Some("set") => self.do_set(words),
                        Some("sleep") => self.do_sleep(words.next()),
                        Some("toggle") => self.do_toggle(words),
                        Some("exit") => return Ok(()),
                        Some("help") => print_interactive_help(),
                        Some("?") => print_interactive_help(),
                        Some(x) => Err(anyhow!("Unknown command: {:?}", x)),
                    } {
                        println!("{}", err);
                        // clean in case the error leaves dirty lines.
                        self.clean();
                    }
                }
                Err(ReadlineError::Interrupted) => return Ok(()),
                Err(ReadlineError::Eof) => return Ok(()),
                Err(err) => return Err(anyhow!(err)),
            }
        }
    }

    fn hold(&mut self) {
        if let Some(period) = self.hold_period {
            self.last_held = true;
            sleep(period);
        }
    }

    fn do_set(&mut self, changes: std::str::SplitAsciiWhitespace) -> Result<()> {
        for lv in changes {
            match parse_key_val::<String, LineValue>(lv) {
                Err(e) => {
                    return Err(anyhow!("Invalid value: {}", e));
                }
                Ok((line_id, value)) => match self.lines.get_mut(&line_id) {
                    Some(line) => {
                        line.value = value.0;
                        line.dirty = true;
                    }
                    None => return Err(anyhow!("Not a requested line: {:?}", line_id)),
                },
            }
        }
        if self.update()? {
            self.hold();
        }
        Ok(())
    }

    fn do_sleep(&mut self, duration: Option<&str>) -> Result<()> {
        match duration {
            Some(period) => match parse_duration(period) {
                Ok(mut d) => {
                    if self.last_held {
                        self.last_held = false;
                        if let Some(period) = self.hold_period {
                            if d < period {
                                // slept longer than that already
                                return Ok(());
                            }
                            d -= period;
                        }
                    }
                    sleep(d);
                }
                Err(e) => return Err(anyhow!("Invalid duration: {}", e)),
            },
            None => return Err(anyhow!("Invalid command: require duration")),
        }
        Ok(())
    }

    fn do_toggle(&mut self, lines: std::str::SplitAsciiWhitespace) -> Result<()> {
        let mut have_lines = false;
        for line_id in lines {
            match self.lines.get_mut(line_id) {
                Some(line) => {
                    line.value = line.value.not();
                    line.dirty = true;
                    have_lines = true;
                }
                None => return Err(anyhow!("Not a requested line: {:?}", line_id)),
            }
        }
        if !have_lines {
            // no lines specified, so toggle all lines
            self.toggle_all_lines();
        }
        if self.update()? {
            self.hold();
        }
        Ok(())
    }

    fn clean(&mut self) {
        for line in self.lines.values_mut() {
            line.dirty = false;
        }
    }

    fn toggle(&mut self, ts: &TimeSequence) -> Result<()> {
        let mut count = 0;
        let hold_period = self.hold_period.unwrap_or(Duration::ZERO);
        loop {
            sleep(max(ts.0[count], hold_period));
            count += 1;
            if count == ts.0.len() - 1 && ts.0[count].is_zero() {
                return Ok(());
            }
            if count == ts.0.len() {
                count = 0;
            }
            self.toggle_all_lines();
            self.update()?;
        }
    }

    fn toggle_all_lines(&mut self) {
        for line in self.lines.values_mut() {
            line.value = line.value.not();
            line.dirty = true;
        }
    }

    fn update(&mut self) -> Result<bool> {
        let mut updated = false;
        for idx in 0..self.chips.len() {
            let chip = &self.chips[idx];
            let mut values = Values::default();
            for line in self.lines.values_mut() {
                if line.dirty && &line.chip == chip {
                    values.set(line.offset, line.value);
                    line.dirty = false;
                }
            }
            if !values.is_empty() {
                self.requests[idx]
                    .set_values(&values)
                    .context("set failed:")?;
                updated = true;
            }
        }
        Ok(updated)
    }
}

fn print_interactive_help() -> Result<()> {
    let cmds = [
        (
            "set <line=value>...",
            "Update the values of the given requested lines",
        ),
        (
            "toggle [line]...",
            "Toggle the values of the given requested lines.\n\
            If no lines are specified then all requested lines are toggled",
        ),
        ("sleep <period>", "Sleep for the specified period"),
        ("help", "Print this help"),
        ("exit", "Exit the program"),
    ];
    println!("{}", "COMMANDS:".yellow());
    for (cmd, help) in cmds {
        print!("\n    {}", cmd.green());
        for line in help.split('\n') {
            println!("\n            {}", line);
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct Line {
    chip: PathBuf,
    offset: Offset,
    value: Value,
    dirty: bool,
}

/// Parse a single key-value pair
fn parse_key_val<T, U>(
    s: &str,
) -> std::result::Result<(T, U), Box<dyn Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`.", s))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

#[derive(Debug)]
struct TimeSequence(Vec<Duration>);

fn parse_time_sequence(s: &str) -> std::result::Result<TimeSequence, ParseDurationError> {
    let mut ts = TimeSequence(Vec::new());
    for period in s.split(',') {
        ts.0.push(parse_duration(period)?);
    }
    Ok(ts)
}

#[derive(Debug)]
struct LineValue(Value);

impl FromStr for LineValue {
    type Err = InvalidLineValue;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let lower_s = s.to_lowercase();
        let v = match lower_s.as_str() {
            "0" | "inactive" | "off" | "false" => Value::Inactive,
            "1" | "active" | "on" | "true" => Value::Active,
            _ => {
                return Err(InvalidLineValue::new(s));
            }
        };
        Ok(LineValue(v))
    }
}

#[derive(Debug)]
struct InvalidLineValue {
    value: String,
}

impl InvalidLineValue {
    pub fn new<S: Into<String>>(value: S) -> InvalidLineValue {
        InvalidLineValue {
            value: value.into(),
        }
    }
}

impl fmt::Display for InvalidLineValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid line value: `{}`.", self.value)
    }
}
impl Error for InvalidLineValue {}

#[derive(Helper, Validator, Hinter, Highlighter)]
struct InteractiveHelper {
    line_names: Vec<String>,
}

impl InteractiveHelper {
    fn complete_set(&self, line: &str, pos: usize, words: SplitWhitespace) -> (usize, Vec<Pair>) {
        let mut candidates = Vec::new();
        let line_values: Vec<&'_ str> = words.collect();
        if line_values.is_empty() {
            for display in self.line_names.iter() {
                candidates.push(line_value_pair(display))
            }
            return (pos, candidates);
        }
        let selected: Vec<&'_ str> = line_values
            .iter()
            .filter(|lv| lv.contains('='))
            .map(|lv| &lv[..lv.find('=').unwrap()])
            .collect();
        let unselected = self
            .line_names
            .iter()
            .filter(|l| !selected.contains(&l.as_str()));
        if line.len() != line.trim_end().len() {
            for display in unselected {
                candidates.push(line_value_pair(display));
            }
            return (pos, candidates);
        }
        let line_value = line_values.last().unwrap();
        let mut vpos = pos;
        match line_value.split_once("=") {
            Some((_, value)) => {
                const VALUES: [&str; 8] =
                    ["active", "inactive", "on", "off", "true", "false", "1", "0"];
                vpos -= value.len();
                for display in VALUES.iter().filter(|v| v.starts_with(value)) {
                    candidates.push(word_pair(display))
                }
            }
            None => {
                let part_name = line_value;
                vpos -= part_name.len();
                for display in unselected.filter(|l| l.starts_with(part_name)) {
                    candidates.push(line_value_pair(display))
                }
            }
        }
        (vpos, candidates)
    }

    fn complete_sleep(
        &self,
        _line: &str,
        pos: usize,
        words: SplitWhitespace,
    ) -> (usize, Vec<Pair>) {
        const UNITS: [&str; 4] = ["s", "ms", "us", "ns"];
        let mut candidates = Vec::new();
        let mut upos = pos;
        let times: Vec<&'_ str> = words.collect();
        if times.len() == 1 {
            let t = times[0];
            match t.find(|c: char| !c.is_ascii_digit()) {
                Some(n) => {
                    let (_num, units) = t.split_at(n);
                    upos -= units.len();
                    for display in UNITS.iter().filter(|u| u.starts_with(units)) {
                        candidates.push(bare_pair(display))
                    }
                }
                None => {
                    for display in UNITS.iter() {
                        candidates.push(bare_pair(display))
                    }
                }
            }
        }
        (upos, candidates)
    }

    fn complete_toggle(
        &self,
        line: &str,
        pos: usize,
        words: SplitWhitespace,
    ) -> (usize, Vec<Pair>) {
        let mut candidates = Vec::new();
        let selected: Vec<&'_ str> = words.collect();
        let unselected = self
            .line_names
            .iter()
            .filter(|l| !selected.contains(&l.as_str()));
        let mut lpos = pos;
        if line.len() != line.trim_end().len() {
            for display in unselected {
                candidates.push(word_pair(display))
            }
        } else {
            let part_word = String::from(*selected.last().unwrap());
            if self.line_names.contains(&part_word) {
                for display in unselected {
                    candidates.push(prespace_word_pair(display))
                }
            } else {
                lpos -= part_word.len();
                for display in unselected.filter(|l| l.starts_with(&part_word)) {
                    candidates.push(word_pair(display))
                }
            }
        }
        (lpos, candidates)
    }
}

impl Completer for InteractiveHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        const CMD_SET: [&str; 5] = ["exit", "help", "set ", "sleep ", "toggle "];
        let cmd_pos = line.len() - line.trim_start().len();
        let mut words = line[..pos].trim_start().split_whitespace();
        let mut candidates = Vec::new();
        match words.next() {
            Some(cmd) => {
                if line.len() > cmd.len() + cmd_pos {
                    return Ok(match cmd {
                        "set" => self.complete_set(line, pos, words),
                        "sleep" => self.complete_sleep(line, pos, words),
                        "toggle" => self.complete_toggle(line, pos, words),
                        _ => (cmd_pos, vec![]),
                    });
                } else {
                    for display in CMD_SET.iter().filter(|x| x.starts_with(cmd)) {
                        candidates.push(cmd_pair(display))
                    }
                }
            }
            None => {
                for display in CMD_SET.iter() {
                    candidates.push(cmd_pair(display))
                }
            }
        }
        Ok((0, candidates))
    }
}

fn cmd_pair(candidate: &str) -> Pair {
    let replacement = String::from(candidate);
    let display = String::from(candidate.trim_end());
    Pair {
        display,
        replacement,
    }
}

fn prespace_word_pair(candidate: &str) -> Pair {
    let display = String::from(candidate);
    let mut replacement = String::from(' ');
    replacement.push_str(candidate);
    Pair {
        display,
        replacement,
    }
}

fn word_pair(candidate: &str) -> Pair {
    let display = String::from(candidate);
    let mut replacement = display.clone();
    replacement.push(' ');
    Pair {
        display,
        replacement,
    }
}

fn line_value_pair(candidate: &str) -> Pair {
    let display = String::from(candidate);
    let mut replacement = display.clone();
    replacement.push('=');
    Pair {
        display,
        replacement,
    }
}

fn bare_pair(candidate: &str) -> Pair {
    let display = String::from(candidate);
    let replacement = display.clone();
    Pair {
        display,
        replacement,
    }
}
