<!--
SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>

SPDX-License-Identifier: CC0-1.0
-->
# gpiocdev-embedded-hal

[![Build Status](https://img.shields.io/github/actions/workflow/status/warthog618/gpiocdev-rs/rust.yml?logo=github&branch=master)](https://github.com/warthog618/gpiocdev-rs/actions/workflows/rust.yml)
[![github](https://img.shields.io/badge/github-warthog618/gpiocdev--rs-8da0cb.svg?logo=github)](https://github.com/warthog618/gpiocdev-rs)
[![crate](https://img.shields.io/crates/v/gpiocdev-embedded-hal.svg?color=fc8d62&logo=rust)](https://crates.io/crates/gpiocdev-embdedded-hal)
![LICENSE](https://img.shields.io/crates/l/gpiocdev-embedded-hal.svg)

A Rust library implementing [embedded-hal](https://crates.io/crates/embedded-hal) traits
for GPIO lines on Linux platforms using the GPIO character device using
[gpiocdev](https://crates.io/crates/gpiocdev).

## Example Usage

Reading line value:

```rust
    use embedded_hal::digital::InputPin;

    // request the line
    let mut pin = gpiocdev_embedded_hal::Input::new("/dev/gpiochip0", 23)?;
    // get the value
    if pin.is_high()? {
         println!("Input is high.");
    }
```

Setting a line:

```rust
    use embedded_hal::digital::OutputPin;
    use embedded_hal::digital::PinState;

    // request the line
    let mut pin = gpiocdev_embedded_hal::Output::new("/dev/gpiochip0", 23, PinState::High)?;

    // do something...

    // change value later
    pin.set_low()?;
```

Requesting a line by name:

```rust
    use embedded_hal::digital::InputPin;

    let mut pin = gpiocdev_embedded_hal::InputPin::from_name("GPIO23")?;
```

Waiting for events on a line:

```rust
    use embedded_hal::digital::InputPin;
    use embedded_hal_async::digital::Wait;

    // request the line
    let mut pin = gpiocdev_embedded_hal::tokio::Input::new("/dev/gpiochip0", 23)?;

    // wait for line edge events
    pin.wait_for_any_edge().await?;
```

Working examples can be found in the [examples](https://github.com/warthog618/gpiocdev-rs/tree/master/embedded-hal/examples) directory.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/warthog618/gpiocdev-rs/blob/master/LICENSES/Apache-2.0.txt) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](https://github.com/warthog618/gpiocdev-rs/blob/master/LICENSES/MIT.txt) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
