# gpiocdev-uapi

[![github](https://img.shields.io/badge/github-warthog618/gpiocdev--rs-8da0cb.svg?style=for-the-badge&logo=github)](https://github.com/warthog618/gpiocdev-rs)
[![crate](https://img.shields.io/crates/v/gpiocdev-uapi.svg?style=for-the-badge&color=fc8d62&logo=rust)](https://crates.io/crates/gpiocdev-uapi)
[![MIT](https://img.shields.io/badge/License-MIT-brightgreen.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)

A thin safe Rust wrapper around the Linux GPIO character device ioctls and file operations.

Rust structs corresponding to the C ABI types are provided, as are functions corresponding to each of the ABI ioctls and chracter device file reads.  Essentially just the Rust version of gpio.h.

Both ABI versions v1 and v2 are supported, though new developments should use v2 unless it is unavailable on the target platform.

The gpiocdev crate provides a higher level abstraction, so you should use that unless you have some particular need to call the ioctls directly.
