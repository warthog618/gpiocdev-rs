// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of reading a single line.

use gpiocdev::line::Bias;
use gpiocdev::Request;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let offset = 22;
    // request the line as an input
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("get-line-value")
        .with_line(offset)
        .as_input()
        .with_bias(Bias::PullUp) // optionally set a pull-up or other attribute
        .request()?;

    let value = req.value(offset)?;
    println!("{offset}={value}");

    Ok(())
}
