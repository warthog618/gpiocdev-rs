// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use gpiod::line::Bias;
use gpiod::request::Request;
use std::result::Result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // request the line as an input
    let mut req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("set_one")
        .with_line(22)
        .as_input()
        .with_bias(Bias::PullUp) // optionally set a pull-up or other attribute
        .request()?;

    let value = req.value(22)?;
    println!("value = {:?}", value);

    Ok(())
}
