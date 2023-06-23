// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Basic example of finding a line with the given name.

fn main() {
    let line_name = "GPIO22";
    match gpiocdev::find_named_line(line_name) {
        Some(gpio22) => {
            println!(
                "{}: {} {}",
                line_name,
                gpio22.chip.file_name().unwrap().to_string_lossy(),
                gpio22.info.offset
            )
        }
        None => println!("line {} not found", line_name),
    }
}
