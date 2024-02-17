<!--
SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>

SPDX-License-Identifier: CC0-1.0
-->
# gpiocdev-rs

[![Build Status](https://img.shields.io/github/actions/workflow/status/warthog618/gpiocdev-rs/rust.yml?logo=github&branch=master)](https://github.com/warthog618/gpiocdev-rs/actions/workflows/rust.yml)

A Rust library for accessing GPIO lines on Linux platforms using the GPIO character device.

This is the equivalent of [libgpiod](https://git.kernel.org/pub/scm/libs/libgpiod/libgpiod.git/), but in pure Rust.

## Crates

[gpiocdev](https://github.com/warthog618/gpiocdev-rs/tree/master/lib) provides a high level API for accessing GPIO lines from Rust applications.

You probably want this one.

[gpiocdev-embedded-hal](https://github.com/warthog618/gpiocdev-rs/tree/master/embedded-hal) provides
[embedded_hal](https://crates.io/crates/embedded-hal) traits for [gpiocdev](https://github.com/warthog618/gpiocdev-rs/tree/master/lib) Requests.

Also provides a simplified interface that may be useful for basic use cases.

[gpiocdev-cli](https://github.com/warthog618/gpiocdev-rs/tree/master/cli) provides a command line tool to access and control GPIO lines.

The tool can be useful for writing scripts accessing the GPIOs, and for debugging GPIO setups.

[gpiocdev-uapi](https://github.com/warthog618/gpiocdev-rs/tree/master/uapi) provides a thin safe Rust wrapper around the character device ioctls.

The [gpiocdev](https://github.com/warthog618/gpiocdev-rs/tree/master/lib) crate provides a higher level abstraction, so you should use that unless you have some particular interest in calling the Linux GPIO ioctls directly.

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
