# gpiocdev-rs

[![MIT](https://img.shields.io/badge/License-MIT-brightgreen.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)

A Rust library for accessing GPIO lines on Linux platforms using the GPIO character device.

This is the equivalent of [libgpiod](https://git.kernel.org/pub/scm/libs/libgpiod/libgpiod.git/), but in pure Rust.

## Crates

[gpiocdev](https://github.com/warthog618/gpiocdev-rs/tree/master/lib) provides a high level API for accessing GPIO lines from Rust applications.

You probably want this one.

[gpiocdev-cli](https://github.com/warthog618/gpiocdev-rs/tree/master/cli) provides a command line tool to access and control GPIO lines.

The tool can be useful for writing scripts accessing the GPIOs, and for debugging GPIO setups.

[gpiocdev-uapi](https://github.com/warthog618/gpiocdev-rs/tree/master/uapi) provides a thin safe Rust wrapper around the character device ioctls.

The gpiocdev crate provides a higher level abstraction, so you should use that unless you have some particular interest in calling the Linux GPIO ioctls directly.

[gpiosim](https://github.com/warthog618/gpiocdev-rs/tree/master/gpiosim) provides GPIO chip simulators for testing users of the GPIO uAPI, such as gpiocdev, or apps that use it.
