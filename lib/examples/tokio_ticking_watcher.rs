// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use futures::StreamExt;
use gpiocdev::{line::EdgeDetection, r#async::tokio::AsyncRequest, request::Request};
use std::time::Instant;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpio22 = gpiocdev::find_named_line("GPIO22").unwrap();
    let req = Request::builder()
        .with_found_line(&gpio22)
        .with_consumer("tokio ticking watcher")
        .with_edge_detection(EdgeDetection::BothEdges)
        .request()?;

    let areq = AsyncRequest::new(req);
    let mut events = areq.edge_events();

    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(500));
    let now = Instant::now();
    loop {
        tokio::select! {
            Some(Ok(event)) = events.next() => {
                println!("{:?}", event);
            }
            _ = ticker.tick() => {
                let elapsed_time = now.elapsed();
                println!("tick {}ms", elapsed_time.as_millis());
            }
        }
    }
}
