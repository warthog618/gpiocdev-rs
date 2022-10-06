// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{self, ActiveLowOpts, BiasOpts, ChipOffset, LineOpts, UapiOpts};
use anyhow::{Context, Result};
use clap::Parser;
use gpiocdev::line::{Offset, Value, Values};
use gpiocdev::request::{Config, Request};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(alias("g"))]
pub struct Opts {
    /// The lines to get
    ///
    /// The lines are identified by name or optionally by offset
    /// if the --chip option is provided.
    #[arg(name = "line", required = true)]
    line: Vec<String>,

    #[command(flatten)]
    line_opts: LineOpts,

    /// Request the line as-is rather than as an input
    ///
    /// If not specified then the lines are explicitly switched to being input lines.
    ///
    /// If specified then the line direction is left as-is, making it
    /// possible to read back the values of output lines.
    #[arg(short, long)]
    as_is: bool,

    #[command(flatten)]
    active_low_opts: ActiveLowOpts,

    #[command(flatten)]
    bias_opts: BiasOpts,

    /// Wait between requesting the lines and reading the values
    ///
    /// This provides time for any bias setting to take effect.
    ///
    /// The period is taken as milliseconds unless otherwise specified.
    #[arg(short = 'p', long, name = "period", value_parser = common::parse_duration)]
    hold_period: Option<Duration>,

    /// Display line values as '0' (inactive) or '1' (active)
    #[arg(long)]
    pub numeric: bool,

    #[command(flatten)]
    uapi_opts: UapiOpts,
}

impl Opts {
    // mutate the config to match the configuration
    fn apply(&self, config: &mut Config) {
        self.active_low_opts.apply(config);
        self.bias_opts.apply(config);
        if !self.as_is {
            config.as_input();
        }
    }
}

pub fn cmd(opts: &Opts) -> Result<bool> {
    let r = common::resolve_lines(&opts.line, &opts.line_opts, opts.uapi_opts.abiv)?;
    let mut requests = Vec::new();
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

        let req = Request::from_config(cfg)
            .on_chip(&ci.path)
            .with_consumer("gpiocdev-get")
            .using_abi_version(common::abi_version_from_opts(opts.uapi_opts.abiv)?)
            .request()
            .context(format!("failed to request lines from {}", ci.name))?;
        requests.push(req);
    }
    if let Some(period) = opts.hold_period {
        thread::sleep(period);
    }
    let mut line_values: HashMap<ChipOffset, Value> = HashMap::new();
    for (idx, ci) in r.chips.iter().enumerate() {
        let mut values = Values::default();
        requests[idx]
            .values(&mut values)
            .context(format!("failed to read values from {}", ci.name))?;
        for (offset, value) in values.iter() {
            line_values.insert(
                ChipOffset {
                    chip_idx: idx,
                    offset: *offset,
                },
                *value,
            );
        }
    }
    let mut print_values = Vec::new();
    for id in &opts.line {
        let value = line_values.get(r.lines.get(id).unwrap()).unwrap();
        if opts.numeric {
            let v: u8 = (*value).into();
            print_values.push(format!("{}", v));
        } else {
            print_values.push(format!("{}={}", id, value));
        }
    }
    println!("{}", print_values.join(" "));

    Ok(true)
}
