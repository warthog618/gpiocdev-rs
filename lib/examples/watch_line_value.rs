// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of watching for edge events on a single line.

use anyhow::Context;
use gpiocdev::line::EdgeDetection;
use gpiocdev::Request;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // request the line with edge detection to receive events
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("watch-line-value")
        .with_line(23)
        .with_edge_detection(EdgeDetection::BothEdges)
        .request()
        .context("Failed to request line")?;

    // then wait for events...

    /* using a blocking read in a loop
    let mut buf = req.new_edge_event_buffer(4);
    loop {
        println!("{:?}", buf.read_event()?);
    }
    */
    // using an iterator
    for event in req.edge_events() {
        println!("{:?}", event?);
    }
    Ok(())
}
