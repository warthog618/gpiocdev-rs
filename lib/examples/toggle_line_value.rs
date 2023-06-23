// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of toggling a single line.

use anyhow::Context;
use gpiocdev::line::Value;
use gpiocdev::Request;
use std::result::Result;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let offset = 22;
    let mut value = Value::Active;

    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("toggle-line-values")
        .with_line(offset)
        .as_output(value)
        .request()
        .context("Failed to request line")?;

    loop {
        println!("{}={:?}", offset, value);
        thread::sleep(Duration::from_millis(500));
        value = value.not();
        req.set_value(offset, value)
            .context("Failed to set value")?;
    }
}
