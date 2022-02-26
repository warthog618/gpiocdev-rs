// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    abi_version_from_opts, parse_chip_path, parse_duration, ActiveLowOpts, BiasOpts, EdgeOpts,
    UapiOpts,
};
use anyhow::{Context, Result};
use clap::Parser;
use gpiod::line::Offset;
use gpiod::request::{Builder, Config};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Parser)]
#[clap(alias("mon"))]
pub struct Opts {
    /// The chip to monitor.
    #[clap(required(true), parse(from_os_str = parse_chip_path))]
    chip: PathBuf,
    /// The set of lines to monitor.
    #[clap(min_values = 1, required = true)]
    lines: Vec<Offset>,
    #[clap(flatten)]
    active_low_opts: ActiveLowOpts,
    #[clap(flatten)]
    bias_opts: BiasOpts,
    #[clap(flatten)]
    edge_opts: EdgeOpts,
    /// The debounce period for the input lines.
    #[clap(short, long, default_value = "0", parse(try_from_str = parse_duration))]
    debounce_period: Duration,
    /// Exit after n events.
    #[clap(short, long)]
    num_events: Option<u32>,
    #[clap(flatten)]
    uapi_opts: UapiOpts,
}

impl Opts {
    // mutate the config to match the configuration
    fn apply<'b>(&self, config: &'b mut Config) -> &'b mut Config {
        if !self.debounce_period.is_zero() {
            config.with_debounce_period(self.debounce_period);
        }
        self.active_low_opts.apply(config);
        self.bias_opts.apply(config);
        self.edge_opts.apply(config);
        config.with_lines(&self.lines)
    }
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let mut cfg = Config::new();
    opts.apply(&mut cfg);
    let mut req = Builder::from_config(cfg)
        .on_chip(&opts.chip)
        .with_consumer("gpiodctl-monitor")
        .using_abi_version(abi_version_from_opts(opts.uapi_opts.abiv)?)
        .request()
        .context("Failed to request lines.")?;

    let mut count = 0;
    let mut buf = req.new_edge_event_buffer(1);
    loop {
        let edge = buf.read_event(&mut req).context("Failed to read event.")?;
        println!("{:?}", edge);
        if let Some(limit) = opts.num_events {
            count += 1;
            if count >= limit {
                println!("Exited after {} events", count);
                return Ok(());
            }
        }
    }
}
