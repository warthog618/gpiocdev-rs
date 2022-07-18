// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use anyhow::Context;
use gpiocdev::line::EdgeDetection;
use gpiocdev::request::Request;
use std::result::Result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("watcher")
        .with_line(23)
        .with_edge_detection(EdgeDetection::BothEdges)
        .request()
        .context("Failed to request line")?;

    /* loop version
    let mut buf = req.new_edge_event_buffer(4);
    loop {
        println!("{:?}", buf.read_event()?);
    }
    */
    for event in req.edge_events()? {
        println!("{:?}", event?);
    }
    Ok(())
}
