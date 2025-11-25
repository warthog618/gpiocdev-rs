// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Example of asynchronously watching for edges on a single line using tokio
// while performing another async task.

use futures::StreamExt;
use gpiocdev::{line::EdgeDetection, tokio::AsyncRequest, Request};
use std::time::Instant;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpio22 = gpiocdev::find_named_line("GPIO22").expect("line GPIO22 should exist");
    let req = Request::builder()
        .with_found_line(&gpio22)
        .with_consumer("tokio-ticking-watch-line-value")
        .with_edge_detection(EdgeDetection::BothEdges)
        .request()?;

    let areq = AsyncRequest::new(req);
    let mut events = areq.edge_events();

    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(500));
    let now = Instant::now();
    loop {
        tokio::select! {
            Some(Ok(event)) = events.next() => {
                println!("{event:?}");
            }
            _ = ticker.tick() => {
                let elapsed_time = now.elapsed();
                println!("tick {}ms", elapsed_time.as_millis());
            }
        }
    }
}
