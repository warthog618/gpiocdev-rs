// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    abi_version_from_opts, parse_chip_path, parse_duration, ActiveLowOpts, BiasOpts, DriveOpts,
    UapiOpts,
};
use anyhow::{Context, Result};
use gpiod::line::{Offset, Value, Values};
use gpiod::request::{Builder, Config};
use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Opts {
    /// The chip to set.
    #[structopt(required = true, parse(from_os_str = parse_chip_path))]
    chip: PathBuf,
    /// The line values in offset=value format.
    #[structopt(required = true, parse(try_from_str = parse_key_val))]
    values: Vec<(Offset, LineValue)>,
    #[structopt(flatten)]
    active_low_opts: ActiveLowOpts,
    #[structopt(flatten)]
    bias_opts: BiasOpts,
    #[structopt(flatten)]
    drive_opts: DriveOpts,
    /// Set the lines then interactively wait for additional set commands on the requested lines.
    #[structopt(short, long)]
    interactive: bool,
    /// The minimum period to hold at the requested the lines before exiting.
    #[structopt(short = "p", long, default_value = "0", parse(try_from_str = parse_duration))]
    hold_period: Duration,
    #[structopt(flatten)]
    uapi_opts: UapiOpts,
}

impl Opts {
    // mutate the config to match the configuration
    fn apply<'b>(&self, config: &'b mut Config) -> &'b mut Config {
        for (l, v) in &self.values {
            config.with_line(*l).as_output(v.0);
        }
        self.active_low_opts.apply(config);
        self.bias_opts.apply(config);
        self.drive_opts.apply(config)
    }
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let mut cfg = Config::new();
    opts.apply(&mut cfg);
    let mut req = Builder::from_config(cfg)
        .on_chip(&opts.chip)
        .with_consumer("gpiodctl-set")
        .using_abi_version(abi_version_from_opts(opts.uapi_opts.abiv)?)
        .request()
        .context("Failed to request and set lines.")?;

    if !opts.hold_period.is_zero() {
        sleep(opts.hold_period);
    }
    if opts.interactive {
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        loop {
            let mut buffer = String::new();
            print!("gpiodctl-set> ");
            std::io::stdout().flush().unwrap();
            if handle.read_line(&mut buffer)? == 0 {
                println!();
                break;
            }
            match parse_line_values(&buffer) {
                Err(e) => println!("{}", e),
                Ok(values) => {
                    req.set_values(&values).context("Failed to set values.")?;
                    if !opts.hold_period.is_zero() {
                        sleep(opts.hold_period);
                    }
                }
            }
        }
    }
    Ok(())
}

fn parse_line_values(s: &str) -> std::result::Result<Values, Box<dyn Error>> {
    let mut values = Values::default();
    for lv in s.trim().split_ascii_whitespace() {
        match parse_key_val::<u32, LineValue>(lv) {
            Err(e) => return Err(e),
            Ok((offset, value)) => values.set(offset, value.0),
        }
    }
    Ok(values)
}

/// Parse a single key-value pair
fn parse_key_val<T, U>(s: &str) -> std::result::Result<(T, U), Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + 'static,
    U: std::str::FromStr,
    U::Err: Error + 'static,
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
            "0" | "inactive" | "false" => Value::Inactive,
            "1" | "active" | "true" => Value::Active,
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
