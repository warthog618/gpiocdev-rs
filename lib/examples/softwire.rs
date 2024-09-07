// SPDX-FileCopyrightText: 2024 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic piece of wire example mirroring an input to an output.

use anyhow::Context;
use gpiocdev::line::{EdgeDetection, EdgeKind, Value};
use gpiocdev::Request;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("softwire")
        .with_line(22)
        .with_edge_detection(EdgeDetection::BothEdges)
        .request()
        .context("Failed to request input")?;

    // copy value to output
    let value = input.value(22)?;
    let output = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("softwire")
        .with_line(23)
        .as_output(value)
        .request()
        .context("Failed to request output")?;

    // track value
    for event in input.edge_events() {
        let value = match event?.kind {
            EdgeKind::Rising => Value::Active,
            EdgeKind::Falling => Value::Inactive,
        };
        output.set_lone_value(value)?;
    }
    Ok(())
}
