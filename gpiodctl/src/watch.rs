// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{chip_from_opts, find_lines, LineOpts, UapiOpts};
use anyhow::{Context, Result};
use clap::Parser;

#[derive(Debug, Parser)]
pub struct Opts {
    #[clap(flatten)]
    line_opts: LineOpts,
    /// The lines to watch.
    ///
    /// The lines are identified by name or optionally by
    /// offset if the --chip option is specified.
    #[clap(name = "line", min_values = 1, required = true)]
    lines: Vec<String>,
    #[clap(flatten)]
    uapi_opts: UapiOpts,
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let (lines, chips) = find_lines(&opts.lines, &opts.line_opts, opts.uapi_opts.abiv)?;
    if chips.len() > 1 {
        panic!("presently only support watching lines on one chip");
    }
    // needs multi-threading or async - so will come back to this...
    //    for chip in &chips { ...
    let chip = chip_from_opts(&chips[0], opts.uapi_opts.abiv)?;

    for offset in lines
        .values()
        .filter(|co| co.chip == chips[0])
        .map(|co| co.offset)
    {
        chip.watch_line_info(offset)
            .context("Failed to watch lines.")?;
    }

    for change in chip.info_change_events()? {
        println!("{:?}", change?);
    }
    Ok(())
}
