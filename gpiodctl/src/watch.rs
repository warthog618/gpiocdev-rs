// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use super::common::{abi_version_from_opts, parse_chip_path, UapiOpts};
use anyhow::{Context, Result};
use gpiod::chip::Chip;
use std::io::Read;
use std::path::PathBuf;
use clap::Parser;

#[derive(Debug, Parser)]
pub struct Opts {
    /// The chip to watch
    #[clap(required = true, parse(from_os_str = parse_chip_path))]
    chip: PathBuf,
    /// The set of lines to watch.
    #[clap(min_values = 1, required = true)]
    lines: Vec<u32>,
    #[clap(flatten)]
    uapi_opts: UapiOpts,
}

pub fn cmd(opts: &Opts) -> Result<()> {
    let mut chip = Chip::from_path(&opts.chip)?;
    chip.using_abi_version(abi_version_from_opts(opts.uapi_opts.abiv)?);

    for offset in &opts.lines {
        chip.watch_line_info(*offset)
            .context("Failed to watch lines.")?;
    }

    let mut buf = Vec::with_capacity(chip.line_info_change_event_size());
    buf.resize(buf.capacity(), 0);
    loop {
        let n = chip.read(&mut buf)?;
        assert_eq!(n, chip.line_info_change_event_size());
        let change = chip
            .line_info_change_event_from_buf(&buf)
            .context("Failed to read event.")?;
        println!("{:?}", change);
    }
}
