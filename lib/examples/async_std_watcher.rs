// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use anyhow::Context;
use gpiocdev::line::{EdgeDetection, Values};
use gpiocdev::async_io::AsyncRequest;
use gpiocdev::Request;
use std::result::Result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let req = AsyncRequest::new(
        Request::builder()
            .on_chip("/dev/gpiochip0")
            .with_consumer("async-std watcher")
            .with_line(23)
            .with_edge_detection(EdgeDetection::BothEdges)
            .request()
            .context("Failed to request line")?,
    );
    let mut values = Values::from_offsets(&[23]);
    async_std::task::block_on(async {
        while let Ok(event) = req.read_edge_event().await {
            println!("{:?} {:?}", event, req.as_ref().values(&mut values));
        }
    });
    Ok(())
}
