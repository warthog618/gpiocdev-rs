// SPDX-FileCopyrightText: 2024 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of reading a single line.

use embedded_hal::digital::InputPin;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pin = gpiocdev_embedded_hal::InputPin::new("/dev/gpiochip0", 22)?;

    if pin.is_high()? {
        println!("pin is high");
    } else {
        println!("pin is low");
    }

    Ok(())
}
