// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    abi_version_from_opts, parse_chip_path, parse_duration, ActiveLowOpts, BiasOpts, UapiOpts,
};
use anyhow::{Context, Result};
use gpiod::line::{Offset, Values};
use gpiod::request::{Builder, Config};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Opts {
    /// The chip to interrogate.
    #[structopt(required = true, parse(from_os_str = parse_chip_path))]
    chip: PathBuf,
    /// The set of lines to get.
    #[structopt(min_values = 1, required = true)]
    lines: Vec<Offset>,
    #[structopt(flatten)]
    active_low_opts: ActiveLowOpts,
    #[structopt(flatten)]
    bias_opts: BiasOpts,
    #[structopt(short, long, default_value = "0", parse(try_from_str = parse_duration))]
    /// A settling period between requesting the lines and reading, to allow bias to take effect.
    settling_period: Duration,
    #[structopt(flatten)]
    uapi_opts: UapiOpts,
}

impl Opts {
    // mutate the config to match the configuration
    fn apply<'b>(&self, config: &'b mut Config) -> &'b mut Config {
        self.active_low_opts.apply(config);
        self.bias_opts.apply(config);
        config.with_lines(&self.lines).as_input()
    }
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let mut cfg = Config::new();
    opts.apply(&mut cfg);
    let mut req = Builder::from_config(cfg)
        .on_chip(&opts.chip)
        .with_consumer("gpiodctl-get")
        .using_abi_version(abi_version_from_opts(opts.uapi_opts.abiv)?)
        .request()
        .context("Failed to request lines.")?;
    if !opts.settling_period.is_zero() {
        sleep(opts.settling_period);
    }
    let mut values = Values::from_lines(&opts.lines);
    req.values(&mut values).context("Failed to read values.")?;
    let last = opts.lines[opts.lines.len() - 1];
    for offset in &opts.lines {
        print!("{}={:?}", offset, values.get(*offset).unwrap());
        if *offset == last {
            println!();
        } else {
            print!(" ")
        }
    }
    Ok(())
}
