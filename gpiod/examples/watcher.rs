// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use gpiod::line::EdgeDetection;
use gpiod::request::Builder;

fn main() {
    match Builder::new()
        .on_chip("/dev/gpiochip0")
        .with_consumer("watcher")
        .with_line(23)
        .with_edge_detection(EdgeDetection::BothEdges)
        .request()
    {
        Ok(mut req) => {
            let mut buf = req.new_edge_event_buffer(4);
            loop {
                match buf.read_event(&mut req) {
                    Ok(edge) => println!("{:?}", edge),
                    Err(e) => {
                        eprintln!("Failed to read edge event: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Err(e) => eprintln!("Failed to request line: {}", e),
    }
}
