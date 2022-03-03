// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    abi_version_from_opts, find_lines, parse_duration, ActiveLowOpts, BiasOpts, DriveOpts,
    LineOpts, UapiOpts,
};
use anyhow::{Context, Result};
use clap::Parser;
use gpiod::line::{Offset, Value, Values};
use gpiod::request::{Builder, Config, Request};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;

#[derive(Debug, Parser)]
pub struct Opts {
    /// The line values in name=value format or optionally offset=value
    /// format if the --chip option is provided.
    #[clap(required = true, parse(try_from_str = parse_key_val))]
    values: Vec<(String, LineValue)>,
    #[clap(flatten)]
    line_opts: LineOpts,
    #[clap(flatten)]
    active_low_opts: ActiveLowOpts,
    #[clap(flatten)]
    bias_opts: BiasOpts,
    #[clap(flatten)]
    drive_opts: DriveOpts,
    /// Set the lines then wait for additional set commands on the requested lines.
    #[clap(short, long)]
    interactive: bool,
    /// The minimum period to hold lines at the requested values.
    #[clap(short = 'p', long, default_value = "0", parse(try_from_str = parse_duration))]
    hold_period: Duration,
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
    let mut setter = Setter::default();
    setter.request(opts)?;
    setter.hold();
    if opts.interactive {
        return setter.interact();
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
    hold_period: Duration,
    // Flag indicating if last operation resulted in a hold
    last_held: bool,
}

impl Setter {
    fn request(&mut self, opts: &Opts) -> Result<()> {
        self.hold_period = opts.hold_period;

        let line_names: Vec<String> = opts.values.iter().map(|(l, _v)| l.to_owned()).collect();
        let (lines, chips) = find_lines(&line_names, &opts.line_opts, opts.uapi_opts.abiv)?;
        self.chips = chips;

        // find set of lines for each chip
        for (line_id, v) in &opts.values {
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
            let mut cfg = Config::new();
            opts.apply(&mut cfg);
            for line in self.lines.values() {
                if &line.chip == chip {
                    cfg.with_line(line.offset).as_output(line.value);
                }
            }
            let req = Builder::from_config(cfg)
                .on_chip(&chip)
                .with_consumer("gpiodctl-set")
                .using_abi_version(abi_version_from_opts(opts.uapi_opts.abiv)?)
                .request()
                .context(format!(
                    "Failed to request and set lines on chip {:?}.",
                    chip
                ))?;
            self.requests.push(req);
        }
        Ok(())
    }

    fn interact(&mut self) -> Result<()> {
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        loop {
            let mut buffer = String::new();
            print!("gpiodctl-set> ");
            std::io::stdout().flush().unwrap();
            if handle.read_line(&mut buffer)? == 0 {
                // EOF
                return Ok(());
            }
            let mut words = buffer.trim().split_ascii_whitespace();
            match words.next() {
                None => continue,
                Some("exit") => return Ok(()),
                Some("set") => self.do_set(words),
                Some("sleep") => self.do_sleep(words.next()),
                Some("toggle") => self.do_toggle(words),
                Some(x) => {
                    println!("Unknown command: {:?}", x);
                }
            }
            self.clean();
        }
    }

    fn hold(&mut self) {
        if !self.hold_period.is_zero() {
            self.last_held = true;
            sleep(self.hold_period);
        }
    }

    fn do_set(&mut self, changes: std::str::SplitAsciiWhitespace) {
        for lv in changes {
            match parse_key_val::<String, LineValue>(lv) {
                Err(e) => {
                    println!("Invalid value: {}", e);
                    return;
                }
                Ok((line_id, value)) => match self.lines.get_mut(&line_id) {
                    Some(line) => {
                        line.value = value.0;
                        line.dirty = true;
                    }
                    None => {
                        println!("Unknown line: {:?}", line_id);
                        return;
                    }
                },
            }
        }
        self.update();
    }

    fn do_sleep(&mut self, duration: Option<&str>) {
        match duration {
            Some(period) => match parse_duration(period) {
                Ok(mut d) => {
                    if self.last_held {
                        self.last_held = false;
                        if d < self.hold_period {
                            // slept longer than that already
                            return;
                        }
                        d -= self.hold_period;
                    }
                    sleep(d);
                }
                Err(e) => println!("Invalid duration: {}", e),
            },
            None => println!("Invalid command: require duration"),
        }
    }

    fn do_toggle(&mut self, lines: std::str::SplitAsciiWhitespace) {
        let mut total = true;
        for line_id in lines {
            match self.lines.get_mut(line_id) {
                Some(line) => {
                    line.value = line.value.toggle();
                    line.dirty = true;
                    total = false;
                }
                None => {
                    println!("Unknown line: {:?}", line_id);
                    return;
                }
            }
        }
        if total {
            // no lines specified, so toggle all lines
            for line in self.lines.values_mut() {
                line.value = line.value.toggle();
                line.dirty = true;
            }
        }
        self.update();
    }

    fn clean(&mut self) {
        for line in self.lines.values_mut() {
            line.dirty = false;
        }
    }

    fn update(&mut self) {
        for idx in 0..self.chips.len() {
            let chip = &self.chips[idx];
            let mut values = Values::default();
            for line in self.lines.values() {
                if line.dirty && &line.chip == chip {
                    values.set(line.offset, line.value);
                }
            }
            if !values.is_empty() {
                if let Err(err) = self.requests[idx].set_values(&values) {
                    println!("set failed: {}", err);
                    return;
                }
            }
        }
    }
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
struct LineValue(Value);

impl FromStr for LineValue {
    type Err = InvalidLineValue;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let lower_s = s.to_lowercase();
        let v = match lower_s.as_str() {
            "0" | "inactive" | "off" => Value::Inactive,
            "1" | "active" | "on" => Value::Active,
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
