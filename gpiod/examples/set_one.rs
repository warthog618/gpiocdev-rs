// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use gpiod::line::Value;
use gpiod::request::Builder;
use std::result::Result;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // request the line and set its value
    let req = Builder::new()
        .on_chip("/dev/gpiochip0")
        .with_consumer("set_one")
        .with_line(22)
        .as_output(Value::Active)
        .request()?;

    // some time later
    sleep(Duration::from_secs(2));
    // change the value...
    req.set_value(22, Value::Inactive)?;

    Ok(())
}
