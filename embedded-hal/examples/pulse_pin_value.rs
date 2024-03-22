// SPDX-FileCopyrightText: 2024 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of pulsing a single line.

use embedded_hal::digital::{OutputPin, PinState};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pin = gpiocdev_embedded_hal::OutputPin::new("/dev/gpiochip0", 22, PinState::High)?;

    thread::sleep(Duration::from_secs(2));
    // change the value...
    pin.set_low()?;

    Ok(())
}
