// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Example of asynchronously watching for edges on a single line using async_io.

use anyhow::Context;
use gpiocdev::async_io::AsyncRequest;
use gpiocdev::line::EdgeDetection;
use gpiocdev::Request;
use std::result::Result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let req = AsyncRequest::new(
        Request::builder()
            .on_chip("/dev/gpiochip0")
            .with_consumer("async-io-watch-line-value")
            .with_line(23)
            .with_edge_detection(EdgeDetection::BothEdges)
            .request()
            .context("Failed to request line")?,
    );
    async_io::block_on(async {
        while let Ok(event) = req.read_edge_event().await {
            println!("{:?}", event);
        }
    });
    Ok(())
}
