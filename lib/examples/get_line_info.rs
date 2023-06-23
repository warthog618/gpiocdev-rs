// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of reading the info for a line.

fn main() -> gpiocdev::Result<()> {
    // Example configuration
    let chip_path = "/dev/gpiochip0";
    let offset = 12;

    let chip = gpiocdev::chip::Chip::from_path(chip_path)?;
    let info = chip.line_info(offset)?;
    println!("{:?}", info);
    Ok(())
}
