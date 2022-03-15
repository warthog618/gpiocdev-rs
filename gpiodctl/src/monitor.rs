// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{
    abi_version_from_opts, find_lines, parse_duration, ActiveLowOpts, BiasOpts, EdgeOpts, LineOpts,
    UapiOpts,
};
use anyhow::{Context, Result};
use clap::Parser;
use gpiod::line::Offset;
use gpiod::request::{Config, Request};
use std::time::Duration;

#[derive(Debug, Parser)]
#[clap(alias("mon"))]
pub struct Opts {
    /// The lines to monitor.
    ///
    /// The lines are identified by name or optionally by offset if
    /// the --chip option is specified.
    #[clap(name = "line", min_values = 1, required = true)]
    lines: Vec<String>,
    #[clap(flatten)]
    line_opts: LineOpts,
    #[clap(flatten)]
    active_low_opts: ActiveLowOpts,
    #[clap(flatten)]
    bias_opts: BiasOpts,
    #[clap(flatten)]
    edge_opts: EdgeOpts,
    /// The debounce period for the monitored lines.
    #[clap(short, long, name = "period", parse(try_from_str = parse_duration))]
    debounce_period: Option<Duration>,
    /// Exit after the specified number of events.
    ///
    /// If not specified then the monitor will continue indefinitely.
    #[clap(short, long, name = "num")]
    num_events: Option<u32>,
    #[clap(flatten)]
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
        .with_consumer("gpiodctl-monitor")
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
