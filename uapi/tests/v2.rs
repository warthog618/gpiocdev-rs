// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(feature = "uapi_v2")]
mod common;

#[cfg(feature = "uapi_v2")]
mod v2 {
    use gpiocdev_uapi::v2::*;
    use gpiosim::{Bank, Direction, Level, Simpleton};
    use std::fs;

    mod get_chip_info;
    mod get_line;
    mod get_line_info;
    mod get_line_values;
    mod set_line_config;
    mod set_line_values;
    mod unwatch_line_info;
    mod watch_line_info;
}
