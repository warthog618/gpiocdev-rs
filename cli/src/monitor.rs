// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    abi_version_from_opts, find_lines, parse_duration, ActiveLowOpts, BiasOpts, EdgeOpts, LineOpts,
    UapiOpts,
};
use anyhow::{Context, Result};
use clap::Parser;
use gpiocdev::line::Offset;
use gpiocdev::request::{Config, Request};
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(alias("mon"))]
pub struct Opts {
    /// The lines to monitor.
    ///
    /// The lines are identified by name or optionally by offset if
    /// the --chip option is specified.
    #[arg(name = "line", required = true)]
    lines: Vec<String>,
    #[command(flatten)]
    line_opts: LineOpts,
    #[command(flatten)]
    active_low_opts: ActiveLowOpts,
    #[command(flatten)]
    bias_opts: BiasOpts,
    #[command(flatten)]
    edge_opts: EdgeOpts,
    /// The debounce period for the monitored lines.
    #[arg(short = 'p', long, name = "period", value_parser = parse_duration)]
    debounce_period: Option<Duration>,
    /// Exit after the specified number of events.
    ///
    /// If not specified then the monitor will continue indefinitely.
    #[arg(short, long, name = "num")]
    num_events: Option<u32>,
    #[command(flatten)]
    uapi_opts: UapiOpts,
}

impl Opts {
    // mutate the config to match the configuration
    fn apply<'b>(&self, config: &'b mut Config) -> &'b mut Config {
        if let Some(period) = self.debounce_period {
            config.with_debounce_period(period);
        }
        self.active_low_opts.apply(config);
        self.bias_opts.apply(config);
        self.edge_opts.apply(config)
    }
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let (lines, chips) = find_lines(&opts.lines, &opts.line_opts, opts.uapi_opts.abiv)?;
    if chips.len() > 1 {
        panic!("presently only support monitoring lines on one chip");
    }
    // needs multi-threading or async - so will come back to this...
    //    for chip in &chips { ...
    let chip = &chips[0];
    let mut cfg = Config::default();
    opts.apply(&mut cfg);
    let offsets: Vec<Offset> = lines
        .values()
        .filter(|co| co.chip == *chip)
        .map(|co| co.offset)
        .collect();
    cfg.with_lines(&offsets);
    let req = Request::from_config(cfg)
        .on_chip(&chip)
        .with_consumer("gpiocdev-monitor")
        .using_abi_version(abi_version_from_opts(opts.uapi_opts.abiv)?)
        .request()
        .context("Failed to request lines.")?;

    let mut count = 0;
    let mut buf = req.new_edge_event_buffer(1);
    loop {
        let edge = buf.read_event().context("Failed to read event.")?;
        println!("{:?}", edge);
        if let Some(limit) = opts.num_events {
            count += 1;
            if count >= limit {
                println!("Exited after {} events.", count);
                return Ok(());
            }
        }
    }
}
