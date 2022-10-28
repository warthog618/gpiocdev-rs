// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use gpiocdev::line::Values;
use gpiocdev::request::Request;
use std::result::Result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // request multiple input lines
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("get_many")
        .with_lines(&[18, 23])
        .as_input()
        .request()?;

    // get multiple line values at once
    let mut values = Values::default();
    req.values(&mut values)?;
    println!("values = {:?}", values);

    Ok(())
}
