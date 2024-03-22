// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of pulsing a single line.

use gpiocdev::line::Value;
use gpiocdev::Request;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // request the line and set its value
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("pulse-line-value")
        .with_line(22)
        .as_output(Value::Active)
        .request()?;

    // some time later
    thread::sleep(Duration::from_secs(2));
    // change the value...
    req.set_value(22, Value::Inactive)?;

    Ok(())
}
