<!--
SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>

SPDX-License-Identifier: CC0-1.0
-->
# gpiocdev

[![Build Status](https://img.shields.io/github/workflow/status/warthog618/gpiocdev-rs/Build.svg?logo=github)](https://github.com/warthog618/gpiocdev-rs/actions)
[![github](https://img.shields.io/badge/github-warthog618/gpiocdev--rs-8da0cb.svg?logo=github)](https://github.com/warthog618/gpiocdev-rs)
[![crate](https://img.shields.io/crates/v/gpiocdev.svg?color=fc8d62&logo=rust)](https://crates.io/crates/gpiocdev)
![LICENSE](https://img.shields.io/crates/l/gpiocdev.svg)

A Rust library for accessing GPIO lines on Linux platforms using the GPIO character device.

This is the equivalent of [libgpiod](https://git.kernel.org/pub/scm/libs/libgpiod/libgpiod.git/) but in pure Rust.

The gpiocdev crate provides an API to access GPIOs from Rust applications.

## ABI Compatibility

The library is compatible with the Linux GPIO uAPI, both v1 and v2, including receiving line edge events.

The gpiocdev API provides a unified abstraction for both uAPI versions, but will return an error if v2 features are attempted to be used on a v1-only system.

uAPI v2 specific features include:

- lines with different configurations
- debouncing input lines
- edge detection on multiple lines in one request
- reconfiguring edge detection without releasing the request
- selection of source clock for edge events

Compatibility with either uAPI version can be selected via features, with the default being uAPI v2.  If built with both, the library will automatically detect and use the most current available version, so defaulting to v2 and falling back to v1 if that is unavailable.

The library makes no use of the deprecated **sysfs** GPIO API.

## Async Compatibility

The majority of the GPIO uAPI is effectively non-blocking and so does not require any async specific treatment.

The exceptions are waiting for edge events from [Request](https://docs.rs/gpiocdev/latest/gpiocdev/request/struct.Request.html)s, and info change events from [Chip](https://docs.rs/gpiocdev/latest/gpiocdev/chip/struct.Chip.html)s.
Support for asynchronous wrappers around these are provided through features for the following reactors:

|Reactor|Feature|Module|
|---|---|---|
|tokio|async_tokio|gpiocdev::async::tokio|
|async-io|async_io|gpiocdev::async::async_io|

Additionally, Chips and Requests also expose their underlying file descriptor, which may be used directly with an async reactor.  An example of this is the **gpiocdev-cli** [edges](https://github.com/warthog618/gpiocdev-rs/blob/master/cli/src/edges.rs) command, which can asynchronously wait on multiple lines spread across multiple chips using the [mio](https://crates.io/crates/mio) reactor.

## Example Usage

Requesting a line by name:

```rust
    let led0 = gpiocdev::find_named_line("LED0").unwrap();
    let req = Request::builder()
        .with_found_line(&led0)
        .as_output(Value::Active)
        .request()?;

    // change value later
    req.set_value(led0.offset, Value::Inactive)
```

Getting a line value:

```rust
    // request the line
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_line(23)
        .as_input()
        .request()?;
    // get the value
    let value = req.value(23)?;
```

Setting a line:

```rust
    // request the line and set its value
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_line(22)
        .as_output(Value::Active)
        .request()?;

    // do something...

    // change value later
    req.set_value(22, Value::Inactive)
```

Waiting for events on a line:

```rust
    // request the line
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_line(23)
        .with_edge_detection(EdgeDetection::BothEdges)
        .request()?;

    // wait for line edge events
    for event in req.edge_events() {
        println!("{:?}", event?);
    }
```

Multiple lines may be selected in a single request, and then be operated on as a unit.

Getting multiple lines:

```rust
    // request multiple input lines
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_lines(&[18,23])
        .as_input()
        .request()?;
    // get multiple line values at once
    let mut values = Values::default();
    req.values(&mut values)?;
```

Setting multiple lines:

```rust
    // request multiple output lines
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_lines(&[17,22])
        .as_output(Value::Active)
        .request()?;
    // set multiple line values at once
    let mut values = Values::default();
    values.set(17, Value::Inactive);
    values.set(12, Value::Active);
    req.set_values(&values)?;
```

All line attributes available via the kernel GPIO interface, such as pull-ups and debounce etc, can also be set:

```rust
    // request the line
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_consumer("myapp")
        .with_line(23)
        .as_input()
        .as_active_low()
        .with_bias(Bias::PullUp)
        .request()?;
    // get the value
    let value = req.value(23)?;
```

A good starting point to learn more is the [gpiocdev::request::Builder](https://docs.rs/gpiocdev/latest/gpiocdev/request/struct.Builder.html).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE/Apache-2.0.txt) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE/MIT.txt) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
