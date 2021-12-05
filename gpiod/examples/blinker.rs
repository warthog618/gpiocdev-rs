// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use gpiod::line::Value;
use gpiod::request::Builder;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let mut value = Value::Active;

    match Builder::new()
        .on_chip("/dev/gpiochip0")
        .with_consumer("blinker")
        .with_line(22)
        .as_output(value)
        .request()
    {
        Ok(mut req) => loop {
            value = value.toggle();
            println!("{:?}", value);
            if let Err(e) = req.set_value(22, value) {
                eprintln!("Failed to toggle value: {}", e);
                std::process::exit(1);
            }
            sleep(Duration::from_millis(500));
        },
        Err(e) => eprintln!("Failed to request line: {}", e),
    }
}
