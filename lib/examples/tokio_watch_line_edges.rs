// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Example of asynchronously watching for edge events on a single line using tokio.

use anyhow::Context;
use gpiocdev::line::{Bias, EdgeDetection};
use gpiocdev::tokio::AsyncRequest;
use gpiocdev::Request;
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let req = AsyncRequest::new(
        Request::builder()
            .on_chip("/dev/gpiochip0")
            .with_consumer("tokio watcher")
            .with_line(22)
            .with_edge_detection(EdgeDetection::BothEdges)
            .with_bias(Bias::PullUp)
            .with_debounce_period(Duration::from_millis(5))
            .request()
            .context("Failed to request line")?,
    );
    loop {
        let event = req.read_edge_event().await;
        println!("{:?}", event?);
    }
}
