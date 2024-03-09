// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of reading multiple lines.

use gpiocdev::line::Values;
use gpiocdev::Request;
use std::result::Result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // request multiple input lines
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("get-multiple-line-values")
        .with_lines(&[18, 23])
        .as_input()
        .request()?;

    // get all requested line values at once
    let mut values = Values::default();
    req.values(&mut values)?;
    println!("all values = {values:?}");

    // or individually
    let mut values = Values::from_offsets(&[23]);
    req.values(&mut values)?;
    println!("one value = {values:?}");
    Ok(())
}
