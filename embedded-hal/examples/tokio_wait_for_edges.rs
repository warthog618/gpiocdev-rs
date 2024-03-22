// SPDX-FileCopyrightText: 2024 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Example of asynchronously waiting for edge events on a single line using tokio.

use anyhow::Context;
use embedded_hal_async::digital::Wait;
use gpiocdev_embedded_hal::tokio::InputPin;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pin = InputPin::new("/dev/gpiochip0", 23).context("Failed to request line")?;
    async_io::block_on(async {
        loop {
            let _ = pin.wait_for_any_edge().await;
            println!("got edge");
        }
    });
    Ok(())
}
