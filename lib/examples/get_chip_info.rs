// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of reading the info for a chip.

fn main() -> gpiocdev::Result<()> {
    let chip_path = "/dev/gpiochip0";

    let chip = gpiocdev::chip::Chip::from_path(chip_path)?;
    let info = chip.info()?;
    println!("{} [{}] ({} lines)", info.name, info.label, info.num_lines);
    Ok(())
}
