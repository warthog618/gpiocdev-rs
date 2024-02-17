// SPDX-FileCopyrightText: 2024 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Example of asynchronously wait for edges on a single line using async_std.

use anyhow::Context;
use embedded_hal_async::digital::Wait;
use gpiocdev_embedded_hal::async_io::InputPin;
use std::result::Result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pin = InputPin::new("/dev/gpiochip0", 23).context("Failed to request line")?;
    async_std::task::block_on(async {
        loop {
            let _ = pin.wait_for_any_edge().await;
            println!("got edge");
        }
    });
    Ok(())
}
