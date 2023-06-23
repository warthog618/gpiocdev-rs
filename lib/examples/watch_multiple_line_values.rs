// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of watching for edge events on multiple lines.

use anyhow::Context;
use gpiocdev::line::EdgeDetection;
use gpiocdev::Request;
use std::result::Result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let offsets = [23, 12];

    // request the line with edge detection to receive events
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("watch-line-value")
        .with_lines(&offsets)
        .with_edge_detection(EdgeDetection::BothEdges)
        .request()
        .context("Failed to request lines")?;

    // then wait for events...

    // using a blocking read in a loop
    let mut buf = req.new_edge_event_buffer(4);
    loop {
        println!("{:?}", buf.read_event()?);
    }

    /* using an iterator
    for event in req.edge_events() {
        println!("{:?}", event?);
    }
    Ok(())
    */
}
