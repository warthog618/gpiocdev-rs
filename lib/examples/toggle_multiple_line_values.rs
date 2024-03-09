// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of toggling multiple lines.

use anyhow::Context;
use gpiocdev::line::Value;
use gpiocdev::Request;
use std::result::Result;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut values = [(22, Value::Active), (12, Value::Inactive)]
        .into_iter()
        .collect();
    // or equivalently...
    //let mut values = Values::default();
    //values.set(22, Value::Active).set(12, Value::Inactive);

    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("toggle-multiple-line-values")
        .with_output_lines(&values)
        .request()
        .context("Failed to request line")?;

    loop {
        println!("{values:?}");
        thread::sleep(Duration::from_millis(500));
        values.not();
        req.set_values(&values).context("Failed to set values")?;
    }
}
