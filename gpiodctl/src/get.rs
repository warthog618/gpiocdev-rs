// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    abi_version_from_opts, find_lines, parse_duration, ActiveLowOpts, BiasOpts, ChipOffset,
    LineOpts, UapiOpts,
};
use anyhow::{Context, Result};
use clap::Parser;
use gpiod::line::{Offset, Value, Values};
use gpiod::request::{Builder, Config};
use std::collections::HashMap;
use std::thread::sleep;
use std::time::Duration;

#[derive(Debug, Parser)]
pub struct Opts {
    /// The lines to get, identified by name or optionally by offset
    /// if the --chip option is provided.
    #[clap(min_values = 1, required = true)]
    lines: Vec<String>,
    #[clap(flatten)]
    line_opts: LineOpts,
    /// Request the line as-is rather than as an input.
    #[clap(short, long)]
    as_is: bool,
    #[clap(flatten)]
    active_low_opts: ActiveLowOpts,
    #[clap(flatten)]
    bias_opts: BiasOpts,
    #[clap(short = 'p', long, default_value = "0", parse(try_from_str = parse_duration))]
    /// A settling period between requesting the lines and reading, to allow bias to take effect.
    hold_period: Duration,
    #[clap(flatten)]
    uapi_opts: UapiOpts,
}

impl Opts {
    // mutate the config to match the configuration
    fn apply<'b>(&self, config: &'b mut Config) -> &'b mut Config {
        self.active_low_opts.apply(config);
        self.bias_opts.apply(config);
        if !self.as_is {
            config.as_input();
        }
        config
    }
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let (lines, chips) = find_lines(&opts.lines, &opts.line_opts, opts.uapi_opts.abiv)?;
    let mut requests = Vec::new();
    for chip in &chips {
        let mut cfg = Config::new();
        opts.apply(&mut cfg);
        let offsets: Vec<Offset> = lines
            .values()
            .filter(|co| co.chip == *chip)
            .map(|co| co.offset)
            .collect();
        cfg.with_lines(&offsets);

        let req = Builder::from_config(cfg)
            .on_chip(&chip)
            .with_consumer("gpiodctl-get")
            .using_abi_version(abi_version_from_opts(opts.uapi_opts.abiv)?)
            .request()
            .context(format!("Failed to request lines from chip {:?}.", chip))?;
        requests.push(req);
    }
    if !opts.hold_period.is_zero() {
        sleep(opts.hold_period);
    }
    let mut line_values: HashMap<ChipOffset, Value> = HashMap::new();
    for idx in 0..chips.len() {
        let mut values = Values::default();
        requests[idx]
            .values(&mut values)
            .context(format!("Failed to read values from chip {:?}.", chips[idx]))?;
        for (offset, value) in values.iter() {
            line_values.insert(
                ChipOffset {
                    chip: chips[idx].to_owned(),
                    offset: *offset,
                },
                *value,
            );
        }
    }
    let mut print_values = Vec::new();
    for line_id in &opts.lines {
        let value = line_values.get(lines.get(line_id).unwrap()).unwrap();
        print_values.push(format!("{}={:?}", line_id, value));
    }
    println!("{}", print_values.join(" "));

    Ok(())
}
