// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use gpiod::line::{Value, Values};
use gpiod::request::Builder;
use std::result::Result;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // request multiple output lines
    let req = Builder::new()
        .on_chip("/dev/gpiochip0")
        .with_lines(&[17, 22])
        .as_output(Value::Active)
        .request()?;

    // some time later...
    sleep(Duration::from_secs(2));

    // change the values at once...
    let mut values = Values::default();
    values.set(17, Value::Inactive);
    values.set(22, Value::Active);
    req.set_values(&values)?;

    // or individually...
    req.set_value(17, Value::Active)?;
    // and a bit later..
    req.set_value(22, Value::Inactive)?;

    Ok(())
}
