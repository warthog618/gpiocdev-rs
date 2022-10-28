// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use gpiocdev::line::Bias;
use gpiocdev::request::Request;
use std::result::Result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // request the line as an input
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("get_one")
        .with_line(22)
        .as_input()
        .with_bias(Bias::PullUp) // optionally set a pull-up or other attribute
        .request()?;

    let value = req.value(22)?;
    println!("value = {:?}", value);

    Ok(())
}
