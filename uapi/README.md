<!--
SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>

SPDX-License-Identifier: CC0-1.0
-->
# gpiocdev-uapi

[![Build Status](https://img.shields.io/github/workflow/status/warthog618/gpiocdev-rs/Build.svg?logo=github)](https://github.com/warthog618/gpiocdev-rs/actions)
[![github](https://img.shields.io/badge/github-warthog618/gpiocdev--rs-8da0cb.svg?logo=github)](https://github.com/warthog618/gpiocdev-rs)
[![crate](https://img.shields.io/crates/v/gpiocdev-uapi.svg?color=fc8d62&logo=rust)](https://crates.io/crates/gpiocdev-uapi)
![LICENSE](https://img.shields.io/crates/l/gpiocdev-uapi.svg)

A thin safe Rust wrapper around the Linux GPIO character device ioctls and file operations.

Rust structs corresponding to the C ABI types are provided, as are functions corresponding to each of the ABI ioctls and character device file reads.  Essentially just the Rust version of gpio.h.

Both ABI versions v1 and v2 are supported, though new developments should use v2 unless it is unavailable on the target platform.

The [gpiocdev](https://crates.io/crates/gpiocdev) crate provides a higher level abstraction, so you should use that unless you have some particular need to call the ioctls directly.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSES/Apache-2.0.txt) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSES/MIT.txt) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
