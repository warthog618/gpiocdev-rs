// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod editor;
use self::editor::{CommandWords, Editor};

use super::common::{self, emit_error, EmitOpts, ParseDurationError};
use anyhow::{anyhow, bail, Context, Result};
use clap::{Arg, ArgAction, Command, Parser};
use daemonize::Daemonize;
use gpiocdev::line::{Offset, Value, Values};
use gpiocdev::request::{Config, Request};
use std::cmp;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::str::FromStr;
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
    #[arg(value_name = "line=value", required = true, value_parser = parse_line_value, verbatim_doc_comment)]
    line_values: Vec<(String, LineValue)>,

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
    drive_opts: common::DriveOpts,

    /// Set the lines then wait for additional set commands for the requested lines.
    ///
    /// Use the "help" command at the interactive prompt to get help for
    /// the supported commands.
    #[arg(short, long, groups = ["mode", "terminal", "emit"])]
    interactive: bool,

    /// The minimum time period to hold lines at the requested values.
    ///
    /// The period is taken as milliseconds unless otherwise specified.
    #[arg(short = 'p', long, value_name = "period", value_parser = common::parse_duration)]
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
    #[arg(short = 't', long, value_name = "periods", value_parser = parse_time_sequence, group = "mode", verbatim_doc_comment)]
    toggle: Option<TimeSequence>,

    /// Set line values then detach from the controlling terminal.
    #[arg(short = 'z', long, group = "terminal")]
    daemonize: bool,

    /// The consumer label applied to requested lines.
    #[arg(short = 'C', long, value_name = "name", default_value = "gpiocdev-set")]
    consumer: String,

    #[command(flatten)]
    uapi_opts: common::UapiOpts,

    #[command(flatten)]
    emit: common::EmitOpts,
}

impl Opts {
    // mutate the config to match the configuration
    fn apply(&self, config: &mut Config) {
        self.active_low_opts.apply(config);
        self.bias_opts.apply(config);
        self.drive_opts.apply(config);
    }
}

pub fn cmd(opts: &Opts) -> bool {
    match do_cmd(opts) {
        Err(e) => {
            emit_error(&opts.emit, &e);
            false
        }
        Ok(x) => x,
    }
}

fn do_cmd(opts: &Opts) -> Result<bool> {
    let mut setter = Setter {
        hold_period: opts.hold_period,
        ..Default::default()
    };
    if !setter.request(opts)? {
        return Ok(false);
    }
    if opts.banner {
        let line_ids: Vec<String> = opts
            .line_values
            .iter()
            .map(|(l, _v)| l.to_owned())
            .collect();
        print_banner(&line_ids);
    }
    if opts.daemonize {
        Daemonize::new().start()?;
    }
    if let Some(ts) = &opts.toggle {
        return setter.toggle(ts);
    }
    setter.hold();
    if opts.interactive {
        return setter.interact(opts);
    }
    setter.wait();
    Ok(true)
}

fn emit_errors(opts: &EmitOpts, errs: &[anyhow::Error]) {
    for e in errs {
        emit_error(opts, e);
    }
}

#[derive(Default)]
struct Setter {
    // IDs of requested lines - in command line order
    line_ids: Vec<String>,

    // Map from command line name to top level line details
    lines: HashMap<String, Line>,

    // The list of chips containing requested lines
    chips: Vec<common::ChipInfo>,

    // The request on each chip
    requests: Vec<Request>,

    // The minimum period to hold set values before applying the subsequent set
    hold_period: Option<Duration>,

    // Flag indicating if last operation resulted in a hold
    last_held: bool,
}

impl Setter {
    fn request(&mut self, opts: &Opts) -> Result<bool> {
        self.line_ids = opts
            .line_values
            .iter()
            .map(|(l, _v)| l.to_owned())
            .collect();
        let r = common::Resolver::resolve_lines(&self.line_ids, &opts.line_opts, &opts.uapi_opts);
        if !r.errors.is_empty() {
            emit_errors(&opts.emit, &r.errors);
            return Ok(false);
        }
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
            let mut bld = Request::from_config(cfg);
            bld.on_chip(&ci.path).with_consumer(&opts.consumer);
            #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
            bld.using_abi_version(r.abiv);
            let req = bld
                .request()
                .with_context(|| format!("failed to request and set lines on {}", ci.name))?;
            self.requests.push(req);
        }
        Ok(true)
    }

    fn interact(&mut self, opts: &Opts) -> Result<bool> {
        let line_names = opts
            .line_values
            .iter()
            .map(|(l, _v)| l.to_owned())
            .collect();
        let mut rl = Editor::new(line_names, "gpiocdev-set> ")?;
        let mut clcmd = Command::new("gpiocdev")
            .no_binary_name(true)
            .disable_help_flag(true)
            .infer_subcommands(true)
            .override_help(interactive_help())
            .subcommand(
                Command::new("get")
                    .about("Display the current values of the given requested lines")
                    .arg(
                        Arg::new("lines")
                            .required(false)
                            .action(ArgAction::Append)
                            .value_parser(parse_line),
                    ),
            )
            .subcommand(
                Command::new("set")
                    .about("Update the values of the given requested lines")
                    .arg(
                        Arg::new("line_values")
                            .value_name("line=value")
                            .required(true)
                            .action(ArgAction::Append)
                            .value_parser(parse_line_value),
                    ),
            )
            .subcommand(
                Command::new("sleep")
                    .about("Sleep for the specified period")
                    .arg(
                        Arg::new("duration")
                            .required(true)
                            .action(ArgAction::Set)
                            .value_parser(common::parse_duration),
                    ),
            )
            .subcommand(
                Command::new("toggle")
                    .about(
                        "Toggle the values of the given requested lines\n\
            If no lines are specified then all requested lines are toggled.",
                    )
                    .arg(
                        Arg::new("lines")
                            .required(false)
                            .action(ArgAction::Append)
                            .value_parser(parse_line),
                    ),
            )
            .subcommand(Command::new("version").about("Print version"))
            .subcommand(Command::new("exit").about("Exit the program").alias("quit"));
        loop {
            match self.parse_command(&mut clcmd, &rl.readline()?) {
                Ok(am) => {
                    if let Err(err) = self.do_command(am, opts) {
                        println!("{}", err);
                        // clean in case the error leaves dirty lines.
                        self.clean();
                        return Ok(true);
                    }
                }
                Err(err) => {
                    println!("{}", err);
                }
            }
        }
    }

    fn parse_command(&self, cmd: &mut Command, line: &str) -> Result<clap::ArgMatches> {
        let mut words = CommandWords::new(line);
        let mut args = Vec::new();
        while let Some(word) = &words.next() {
            args.push(*word);
        }
        if words.inquote {
            return Err(anyhow!(format!(
                "missing closing quote in '{}'",
                args.last().unwrap()
            )));
        }
        Ok(cmd.try_get_matches_from_mut(args)?)
    }

    fn do_command(&mut self, args: clap::ArgMatches, opts: &Opts) -> Result<()> {
        if let Some((cmd, am)) = args.subcommand() {
            match cmd {
                "get" => {
                    let lines: Vec<String> = am
                        .get_many::<String>("lines")
                        .unwrap_or_default()
                        .cloned()
                        .collect();
                    self.do_get(lines.as_slice(), opts)
                }
                "set" => {
                    let lvs: Vec<(String, LineValue)> = am
                        .get_many::<(String, LineValue)>("line_values")
                        .unwrap()
                        .cloned()
                        .collect();
                    self.do_set(lvs.as_slice())
                }
                "sleep" => {
                    let d: Duration = am.get_one::<Duration>("duration").unwrap().to_owned();
                    self.do_sleep(d)
                }
                "toggle" => {
                    let lines: Vec<String> = am
                        .get_many::<String>("lines")
                        .unwrap_or_default()
                        .cloned()
                        .collect();
                    self.do_toggle(lines.as_slice())
                }
                "exit" => Err(CmdError::Exit().into()),
                "version" => {
                    println!("gpiocdev-set {}", clap::crate_version!());
                    Ok(())
                }
                // help returned as parser error by clap
                _ => Ok(()),
            }
        } else {
            Ok(())
        }
    }

    fn hold(&mut self) {
        if let Some(period) = self.hold_period {
            self.last_held = true;
            thread::sleep(period);
        }
    }

    fn do_get(&mut self, lines: &[String], opts: &Opts) -> Result<()> {
        let mut print_values = Vec::new();
        for id in lines {
            match self.lines.get(id) {
                Some(line) => {
                    print_values.push(format_line_value(&opts.emit, id, line.value));
                }
                None => bail!(CmdError::NotRequestedLine(id.into())),
            }
        }
        if print_values.is_empty() {
            // no lines specified, so return all lines
            for id in &self.line_ids {
                let value = self.lines.get(id).unwrap().value;
                print_values.push(format_line_value(&opts.emit, id, value));
            }
        }
        println!("{}", print_values.join(" "));

        Ok(())
    }

    fn do_set(&mut self, changes: &[(String, LineValue)]) -> Result<()> {
        for (id, value) in changes {
            match self.lines.get_mut(id) {
                Some(line) => {
                    line.value = value.0;
                    line.dirty = true;
                }
                None => bail!(CmdError::NotRequestedLine(id.into())),
            }
        }
        if self.update()? {
            self.hold();
        }
        Ok(())
    }

    fn do_sleep(&mut self, mut d: Duration) -> Result<()> {
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
        Ok(())
    }

    fn do_toggle(&mut self, lines: &[String]) -> Result<()> {
        for id in lines {
            match self.lines.get_mut(id) {
                Some(line) => {
                    line.value = line.value.not();
                    line.dirty = true;
                }
                None => bail!(CmdError::NotRequestedLine(id.into())),
            }
        }
        if lines.is_empty() {
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

    fn toggle(&mut self, ts: &TimeSequence) -> Result<bool> {
        if ts.0.len() == 1 && ts.0[0].is_zero() {
            self.hold();
            return Ok(true);
        }
        let mut count = 0;
        let hold_period = self.hold_period.unwrap_or(Duration::ZERO);
        loop {
            thread::sleep(cmp::max(ts.0[count], hold_period));
            count += 1;
            if count == ts.0.len() - 1 && ts.0[count].is_zero() {
                return Ok(true);
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

    fn wait(&self) {
        // just block on something that should never happen...
        _ = self.requests[0].read_edge_event();
    }
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum CmdError {
    #[error("")]
    Exit(),

    #[error("not a requested line: '{0}'")]
    NotRequestedLine(String),
}

fn interactive_help() -> String {
    let mut help = "COMMANDS:\n".to_owned();

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
        ("version", "Print version"),
        ("exit", "Exit the program"),
    ];
    for (cmd, desc) in cmds {
        let cmd_line = format!("\n    {}", cmd);
        help.push_str(&cmd_line);
        for line in desc.split('\n') {
            let desc_line = format!("\n            {}\n", line);
            help.push_str(&desc_line);
        }
    }
    help
}

fn print_banner(lines: &[String]) {
    use std::io::Write;
    if lines.len() > 1 {
        print!("Setting lines ");

        for l in lines.iter().take(lines.len() - 1) {
            print!("'{}', ", l);
        }

        println!("and '{}'...", lines[lines.len() - 1]);
    } else {
        println!("Setting line '{}'...", lines[0]);
    }
    _ = std::io::stdout().flush();
}

#[derive(Debug, Default)]
struct Line {
    chip_idx: usize,
    offset: Offset,
    value: Value,
    dirty: bool,
}

fn format_line_value(opts: &EmitOpts, id: &str, value: Value) -> String {
    if opts.quoted || id.contains(' ') {
        format!("\"{}\"={}", id, value)
    } else {
        format!("{}={}", id, value)
    }
}

// strips quotes surrounding the whole string.
fn unquoted(s: &str) -> &str {
    if s.starts_with('"') && s.ends_with('"') && s.len() > 1 {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Parse a single line id
fn parse_line(s: &str) -> std::result::Result<String, Box<dyn Error + Send + Sync + 'static>> {
    Ok(unquoted(s).to_string())
}

/// Parse a single line=value pair
fn parse_line_value(s: &str) -> std::result::Result<(String, LineValue), anyhow::Error> {
    let pos = s
        .rfind('=')
        .ok_or_else(|| anyhow!("invalid line=value: no '=' found in '{}'", s))?;
    let ln = unquoted(&s[..pos]);
    if ln.contains('"') {
        bail!("invalid line=value: semi-quoted line name in '{}'", s)
    } else {
        Ok((ln.to_string(), s[pos + 1..].parse()?))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    mod parse {
        #[test]
        fn line() {
            use super::parse_line;
            assert_eq!(parse_line("unquoted").unwrap(), "unquoted".to_string());
            assert_eq!(parse_line("\"semi").unwrap(), "\"semi".to_string());
            assert_eq!(parse_line("\"quoted\"").unwrap(), "quoted".to_string());
        }

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
                parse_line_value("\"quoted\"=false").unwrap(),
                ("quoted".to_string(), LineValue(Value::Inactive))
            );
            assert_eq!(
                parse_line_value("\"quoted\\ name\"=1").unwrap(),
                ("quoted\\ name".to_string(), LineValue(Value::Active))
            );
            assert_eq!(
                parse_line_value("\"quoted=false")
                    .err()
                    .unwrap()
                    .to_string(),
                "invalid line=value: semi-quoted line name in '\"quoted=false'"
            );
            assert_eq!(
                parse_line_value("unquoted\"=false")
                    .err()
                    .unwrap()
                    .to_string(),
                "invalid line=value: semi-quoted line name in 'unquoted\"=false'"
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
                .eq([Duration::ZERO].iter()));
            assert!(parse_time_sequence("1")
                .unwrap()
                .0
                .iter()
                .eq([Duration::from_millis(1)].iter()));
            assert!(parse_time_sequence("2ms")
                .unwrap()
                .0
                .iter()
                .eq([Duration::from_millis(2)].iter()));
            assert!(parse_time_sequence("3us")
                .unwrap()
                .0
                .iter()
                .eq([Duration::from_micros(3)].iter()));
            assert!(parse_time_sequence("4s")
                .unwrap()
                .0
                .iter()
                .eq([Duration::new(4, 0)].iter()));
            assert!(parse_time_sequence("1,2ms,3us,4s,0")
                .unwrap()
                .0
                .iter()
                .eq([
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
