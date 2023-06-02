// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use anyhow::Context;
use gpiocdev::tokio::AsyncRequest;
use gpiocdev::line::{EdgeDetection, Values};
use gpiocdev::request::Request;
use std::result::Result;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let req = AsyncRequest::new(
        Request::builder()
            .on_chip("/dev/gpiochip0")
            .with_consumer("tokio watcher")
            .with_line(23)
            .with_edge_detection(EdgeDetection::BothEdges)
            .request()
            .context("Failed to request line")?,
    );
    let mut values = Values::from_offsets(&[23]);
    loop {
        let event = req.read_edge_event().await;
        println!("{:?} {:?}", event?, req.as_ref().values(&mut values));
    }
}
