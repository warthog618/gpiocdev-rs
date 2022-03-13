// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use anyhow::Context;
use gpiod::line::Value;
use gpiod::request::Request;
use std::result::Result;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut value = Value::Active;

    let mut req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("blinker")
        .with_line(22)
        .as_output(value)
        .request()
        .context("Failed to request line")?;

    loop {
        sleep(Duration::from_millis(500));
        value = value.not();
        println!("{:?}", value);
        req.set_value(22, value).context("Failed to toggle value")?;
    }
}
