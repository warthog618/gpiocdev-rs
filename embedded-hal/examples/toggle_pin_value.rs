// SPDX-FileCopyrightText: 2024 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of toggling a single line.

use embedded_hal::digital::{PinState, StatefulOutputPin};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pin = gpiocdev_embedded_hal::OutputPin::new("/dev/gpiochip0", 22, PinState::High)?;

    loop {
        thread::sleep(Duration::from_millis(500));
        println!("toggle pin...");
        pin.toggle()?;
    }
}
