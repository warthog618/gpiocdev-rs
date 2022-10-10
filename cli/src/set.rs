// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    self, ActiveLowOpts, BiasOpts, ChipInfo, DriveOpts, LineOpts, ParseDurationError, UapiOpts,
};
use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use colored::*;
use daemonize::Daemonize;
use gpiocdev::line::{Offset, Value, Values};
use gpiocdev::request::{Config, Request};
use rustyline::completion::{Completer, Pair};
use rustyline::config::CompletionType;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};
use std::cmp;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::str::{FromStr, SplitWhitespace};
use std::thread;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(alias("s"))]
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
    #[arg(name = "line=value", required = true, value_parser = parse_line_value, verbatim_doc_comment)]
    line_values: Vec<(String, LineValue)>,

    #[command(flatten)]
    line_opts: LineOpts,

    #[command(flatten)]
    active_low_opts: ActiveLowOpts,

    #[command(flatten)]
    bias_opts: BiasOpts,

    #[command(flatten)]
    drive_opts: DriveOpts,

    /// Set the lines then wait for additional set commands for the requested lines.
    ///
    /// Use the "help" command at the interactive prompt to get help for
    /// the supported commands.
    #[arg(short, long, groups = ["mode", "terminal"])]
    interactive: bool,

    /// The minimum time period to hold lines at the requested values.
    ///
    /// The period is taken as milliseconds unless otherwise specified.
    #[arg(short = 'p', long, name = "period", value_parser = common::parse_duration)]
    hold_period: Option<Duration>,

    /// Toggle the lines after the specified time periods.
    ///
    /// The time periods are a comma separated list, and are taken as
    /// milliseconds unless otherwise specified.
    /// The lines are toggled after the period elapses, so the initial time period
    /// applies to the initial line value.
    /// If the final period is not zero, then the sequence repeats.
    ///
    ///  e.g.
    ///      -t 10ms
    ///      -t 100us,200us,100us,150us
    ///      -t 1s,2s,1s,0
    ///
    /// The first two examples repeat, the third exits after 4s.
    ///
    /// A 0s period elsewhere in the sequence is toggled as quickly as possible,
    /// allowing for any specified --hold-period.
    #[arg(short = 't', long, name = "periods", value_parser = parse_time_sequence, group = "mode", verbatim_doc_comment)]
    toggle: Option<TimeSequence>,

    /// Set line values then detach from the controlling terminal.
    #[arg(short = 'z', long, group = "terminal")]
    daemonize: bool,

    /// The consumer label applied to requested lines.
    #[arg(long, name = "consumer", default_value = "gpioset")]
    consumer: String,

    #[command(flatten)]
    uapi_opts: UapiOpts,
}

impl Opts {
    // mutate the config to match the configuration
    fn apply(&self, config: &mut Config) {
        self.active_low_opts.apply(config);
        self.bias_opts.apply(config);
        self.drive_opts.apply(config);
    }
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let mut setter = Setter {
        hold_period: opts.hold_period,
        ..Default::default()
    };
    setter.request(opts)?;
    if opts.daemonize {
        Daemonize::new().start()?;
    }
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
    // IDs of requested lines - in command line order
    line_ids: Vec<String>,

    // Map from command line name to top level line details
    lines: HashMap<String, Line>,

    // The list of chips containing requested lines
    chips: Vec<ChipInfo>,

    // The request on each chip
    requests: Vec<Request>,

    // The minimum period to hold set values before applying the subsequent set
    hold_period: Option<Duration>,

    // Flag indicating if last operation resulted in a hold
    last_held: bool,
}

impl Setter {
    fn request(&mut self, opts: &Opts) -> Result<()> {
        self.line_ids = opts
            .line_values
            .iter()
            .map(|(l, _v)| l.to_owned())
            .collect();
        let r = common::resolve_lines(&self.line_ids, &opts.line_opts, opts.uapi_opts.abiv)?;
        r.validate(&self.line_ids, &opts.line_opts)?;
        self.chips = r.chips;

        // find set of lines for each chip
        for (id, v) in &opts.line_values {
            let co = r.lines.get(id).unwrap();
            self.lines.insert(
                id.to_owned(),
                Line {
                    chip_idx: co.chip_idx,
                    offset: co.offset,
                    value: v.0,
                    dirty: false,
                },
            );
        }

        // request the lines
        for (idx, ci) in self.chips.iter().enumerate() {
            let mut cfg = Config::default();
            opts.apply(&mut cfg);
            for line in self.lines.values() {
                if line.chip_idx == idx {
                    cfg.with_line(line.offset).as_output(line.value);
                }
            }
            let req = Request::from_config(cfg)
                .on_chip(&ci.path)
                .with_consumer(&opts.consumer)
                .using_abi_version(common::abi_version_from_opts(opts.uapi_opts.abiv)?)
                .request()
                .with_context(|| format!("unable to request and set lines on {}.", ci.name))?;
            self.requests.push(req);
        }
        Ok(())
    }

    fn interact(&mut self, opts: &Opts) -> Result<()> {
        use std::io::Write;

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
        let mut rl = Editor::with_config(config)?;
        rl.set_helper(Some(helper));
        let mut stdout = std::io::stdout();
        let prompt = "gpiocdev-set> ";
        loop {
            /*
             * manually print the prompt, as rustyline doesn't if stdout
             * is not a tty? And flush to ensure the prompt and any
             * output buffered from the previous command is sent.
             */
            _ = stdout.write(prompt.as_bytes());
            _ = stdout.flush();
            let readline = rl.readline(prompt);
            match readline {
                Ok(line) => {
                    let mut words = line.trim().split_ascii_whitespace();
                    if let Err(err) = match words.next() {
                        None => continue,
                        Some("get") => self.do_get(words),
                        Some("set") => self.do_set(words),
                        Some("sleep") => self.do_sleep(words.next()),
                        Some("toggle") => self.do_toggle(words),
                        Some("exit") => return Ok(()),
                        Some("help") => print_interactive_help(),
                        Some("?") => print_interactive_help(),
                        Some(x) => Err(anyhow!("unknown command: '{}'", x)),
                    } {
                        println!("{}", err);
                        // clean in case the error leaves dirty lines.
                        self.clean();
                    }
                }
                Err(ReadlineError::Interrupted) => return Ok(()),
                Err(ReadlineError::Eof) => return Ok(()),
                Err(err) => bail!(err),
            }
        }
    }

    fn hold(&mut self) {
        if let Some(period) = self.hold_period {
            self.last_held = true;
            thread::sleep(period);
        }
    }

    fn do_get(&mut self, lines: std::str::SplitAsciiWhitespace) -> Result<()> {
        let mut have_lines = false;
        let mut print_values = Vec::new();
        for id in lines {
            match self.lines.get(id) {
                Some(line) => {
                    print_values.push(format!("{}={}", id, line.value));
                    have_lines = true;
                }
                None => bail!("not a requested line: '{}'", id),
            }
        }
        if !have_lines {
            // no lines specified, so return all lines
            for id in &self.line_ids {
                print_values.push(format!("{}={}", id, self.lines.get(id).unwrap().value));
            }
        }
        println!("{}", print_values.join(" "));

        Ok(())
    }

    fn do_set(&mut self, changes: std::str::SplitAsciiWhitespace) -> Result<()> {
        for lv in changes {
            match parse_line_value(lv) {
                Err(e) => {
                    bail!("invalid value: {}", e);
                }
                Ok((id, value)) => match self.lines.get_mut(&id) {
                    Some(line) => {
                        line.value = value.0;
                        line.dirty = true;
                    }
                    None => bail!("not a requested line: '{}'", id),
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
            Some(period) => match common::parse_duration(period) {
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
                    thread::sleep(d);
                }
                Err(e) => bail!("invalid duration: '{}'", e),
            },
            None => bail!("invalid command: require duration"),
        }
        Ok(())
    }

    fn do_toggle(&mut self, lines: std::str::SplitAsciiWhitespace) -> Result<()> {
        let mut have_lines = false;
        for id in lines {
            match self.lines.get_mut(id) {
                Some(line) => {
                    line.value = line.value.not();
                    line.dirty = true;
                    have_lines = true;
                }
                None => bail!("not a requested line: '{}'", id),
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
            thread::sleep(cmp::max(ts.0[count], hold_period));
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
            let mut values = Values::default();
            for line in self.lines.values_mut() {
                if line.dirty && line.chip_idx == idx {
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
            "get [line]...",
            "Display the current values of the given requested lines",
        ),
        (
            "set <line=value>...",
            "Update the values of the given requested lines",
        ),
        (
            "toggle [line]...",
            "Toggle the values of the given requested lines\n\
            If no lines are specified then all requested lines are toggled.",
        ),
        ("sleep <period>", "Sleep for the specified period"),
        ("help", "Print this help"),
        ("exit", "Exit the program"),
    ];
    println!("{}", "COMMANDS:".underline());
    for (cmd, help) in cmds {
        print!("\n    {}", cmd.bold());
        for line in help.split('\n') {
            println!("\n            {}", line);
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct Line {
    chip_idx: usize,
    offset: Offset,
    value: Value,
    dirty: bool,
}

/// Parse a single line=value pair
fn parse_line_value(
    s: &str,
) -> std::result::Result<(String, LineValue), Box<dyn Error + Send + Sync + 'static>> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid line=value: no '=' found in '{}'", s))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

#[derive(Clone, Debug)]
struct TimeSequence(Vec<Duration>);

fn parse_time_sequence(s: &str) -> std::result::Result<TimeSequence, ParseDurationError> {
    let mut ts = TimeSequence(Vec::new());
    for period in s.split(',') {
        ts.0.push(common::parse_duration(period)?);
    }
    Ok(ts)
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
        write!(f, "invalid line value: '{}'", self.value)
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
        match line_value.split_once('=') {
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
        const CMD_SET: [&str; 6] = ["exit", "get", "help", "set ", "sleep ", "toggle "];
        let cmd_pos = line.len() - line.trim_start().len();
        let mut words = line[..pos].split_whitespace();
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

#[cfg(test)]
mod tests {
    use super::*;

    mod parse {
        #[test]
        fn line_value() {
            use super::{parse_line_value, LineValue};
            use gpiocdev::line::Value;
            assert_eq!(
                parse_line_value("blah=0").unwrap(),
                ("blah".to_string(), LineValue(Value::Inactive))
            );
            assert_eq!(
                parse_line_value("l=1").unwrap(),
                ("l".to_string(), LineValue(Value::Active))
            );
            assert_eq!(
                parse_line_value("l=active").unwrap(),
                ("l".to_string(), LineValue(Value::Active))
            );
            assert_eq!(
                parse_line_value("l=inactive").unwrap(),
                ("l".to_string(), LineValue(Value::Inactive))
            );
            assert_eq!(
                parse_line_value("l=on").unwrap(),
                ("l".to_string(), LineValue(Value::Active))
            );
            assert_eq!(
                parse_line_value("l=off").unwrap(),
                ("l".to_string(), LineValue(Value::Inactive))
            );
            assert_eq!(
                parse_line_value("l=true").unwrap(),
                ("l".to_string(), LineValue(Value::Active))
            );
            assert_eq!(
                parse_line_value("l=false").unwrap(),
                ("l".to_string(), LineValue(Value::Inactive))
            );
            assert_eq!(
                parse_line_value("5").err().unwrap().to_string(),
                "invalid line=value: no '=' found in '5'"
            );
            assert_eq!(
                parse_line_value("blah=3").err().unwrap().to_string(),
                "invalid line value: '3'"
            );
        }

        #[test]
        fn time_sequence() {
            use super::parse_time_sequence;
            use crate::common::ParseDurationError;
            use std::time::Duration;
            assert!(parse_time_sequence("0")
                .unwrap()
                .0
                .iter()
                .eq(vec![Duration::ZERO].iter()));
            assert!(parse_time_sequence("1")
                .unwrap()
                .0
                .iter()
                .eq(vec![Duration::from_millis(1)].iter()));
            assert!(parse_time_sequence("2ms")
                .unwrap()
                .0
                .iter()
                .eq(vec![Duration::from_millis(2)].iter()));
            assert!(parse_time_sequence("3us")
                .unwrap()
                .0
                .iter()
                .eq(vec![Duration::from_micros(3)].iter()));
            assert!(parse_time_sequence("4s")
                .unwrap()
                .0
                .iter()
                .eq(vec![Duration::new(4, 0)].iter()));
            assert!(parse_time_sequence("1,2ms,3us,4s,0")
                .unwrap()
                .0
                .iter()
                .eq(vec![
                    Duration::from_millis(1),
                    Duration::from_millis(2),
                    Duration::from_micros(3),
                    Duration::new(4, 0),
                    Duration::ZERO
                ]
                .iter()));
            assert_eq!(
                parse_time_sequence("5ns").unwrap_err(),
                ParseDurationError::Units("5ns".to_string())
            );
            assert_eq!(
                parse_time_sequence("bad").unwrap_err(),
                ParseDurationError::NoDigits("bad".to_string())
            );
        }
    }
}
