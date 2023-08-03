// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::common::{self, format_error, EmitOpts};
use anyhow::anyhow;
use clap::Parser;
use gpiocdev::line::{Offset, Value, Values};
use gpiocdev::request::{Config, Request};
#[cfg(feature = "serde")]
use serde_derive::Serialize;
use std::thread;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(alias("g"))]
pub struct Opts {
    /// The lines to get
    ///
    /// The lines are identified by name or optionally by offset
    /// if the --chip option is provided.
    #[arg(value_name = "line", required = true)]
    line: Vec<String>,

    #[command(flatten)]
    line_opts: common::LineOpts,

    /// Request the line as-is rather than as an input
    ///
    /// If not specified then the lines are explicitly switched to being input lines.
    ///
    /// If specified then the line direction is left as-is, making it
    /// possible to read back the values of output lines.
    #[arg(short, long)]
    as_is: bool,

    #[command(flatten)]
    active_low_opts: common::ActiveLowOpts,

    #[command(flatten)]
    bias_opts: common::BiasOpts,

    /// Wait between requesting the lines and reading the values
    ///
    /// This provides time for any bias setting to take effect.
    ///
    /// The period is taken as milliseconds unless otherwise specified.
    #[arg(short = 'p', long, value_name = "period", value_parser = common::parse_duration)]
    hold_period: Option<Duration>,

    /// Display line values as '0' (inactive) or '1' (active)
    #[arg(long, group = "emit")]
    pub numeric: bool,

    #[command(flatten)]
    uapi_opts: common::UapiOpts,

    /// The consumer label applied to requested lines.
    #[arg(short = 'C', long, value_name = "name", default_value = "gpiocdev-get")]
    consumer: String,

    #[command(flatten)]
    emit: common::EmitOpts,
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

pub fn cmd(opts: &Opts) -> bool {
    let res = do_cmd(opts);
    res.emit(opts);
    res.errors.is_empty()
}

fn do_cmd(opts: &Opts) -> CmdResult {
    let mut res = CmdResult {
        ..Default::default()
    };
    let r = common::Resolver::resolve_lines(&opts.line, &opts.line_opts, &opts.uapi_opts);
    for e in &r.errors {
        res.push_error(&opts.emit, e);
    }
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

        let mut bld = Request::from_config(cfg);
        bld.on_chip(&ci.path).with_consumer(&opts.consumer);
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        bld.using_abi_version(r.abiv);
        match bld.request() {
            Ok(req) => {
                requests.push(Some(req));
            }
            Err(e) => {
                requests.push(None);
                res.push_error(
                    &opts.emit,
                    &anyhow!(e).context(format!(
                        "failed to request lines {:?} from {}",
                        offsets, ci.name
                    )),
                );
            }
        }
    }
    if let Some(period) = opts.hold_period {
        thread::sleep(period);
    }
    for (idx, ci) in r.chips.iter().enumerate() {
        if let Some(req) = &requests[idx] {
            let mut values = Values::default();
            match req.values(&mut values) {
                Ok(()) => {
                    for line in r.lines.iter().filter(|l| l.1.chip_idx == idx) {
                        res.values.push(LineValue {
                            id: line.0.to_string(),
                            value: values.get(line.1.offset).unwrap(),
                        });
                    }
                }
                Err(e) => {
                    res.push_error(
                        &opts.emit,
                        &anyhow!(e).context(format!("failed to read values from {}", ci.name)),
                    );
                }
            }
        }
    }
    res
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(Serialize))]
struct CmdResult {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    values: Vec<LineValue>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    errors: Vec<String>,
}
impl CmdResult {
    fn emit(&self, opts: &Opts) {
        #[cfg(feature = "json")]
        if opts.emit.json {
            println!("{}", serde_json::to_string(self).unwrap());
            return;
        }
        self.print(opts);
    }

    fn push_error(&mut self, opts: &EmitOpts, e: &anyhow::Error) {
        self.errors.push(format_error(opts, e))
    }

    fn print(&self, opts: &Opts) {
        let mut print_values = Vec::new();
        let mut seen_lines = Vec::new();
        for id in &opts.line {
            if seen_lines.contains(id) {
                continue;
            }
            seen_lines.push(id.clone());
            for lv in &self.values {
                if &lv.id == id {
                    print_values.push(if opts.numeric {
                        let v: u8 = lv.value.into();
                        format!("{}", v)
                    } else if opts.emit.quoted {
                        format!("\"{}\"={}", lv.id, lv.value)
                    } else {
                        format!("{}={}", lv.id, lv.value)
                    });
                    break;
                }
            }
        }
        if !print_values.is_empty() {
            println!("{}", print_values.join(" "));
        }
        for e in &self.errors {
            eprintln!("{}", e);
        }
    }
}

struct LineValue {
    id: String,
    value: Value,
}
#[cfg(feature = "serde")]
impl serde::Serialize for LineValue {
    fn serialize<S>(&self, serializer: S) -> anyhow::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut s = serializer.serialize_map(Some(1))?;
        s.serialize_entry(&self.id, &self.value)?;
        s.end()
    }
}
