<!--
SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>

SPDX-License-Identifier: CC0-1.0
-->
# gpiocdev

[![Build Status](https://img.shields.io/github/actions/workflow/status/warthog618/gpiocdev-rs/rust.yml?logo=github&branch=master)](https://github.com/warthog618/gpiocdev-rs/actions/workflows/rust.yml)
[![github](https://img.shields.io/badge/github-warthog618/gpiocdev--rs-8da0cb.svg?logo=github)](https://github.com/warthog618/gpiocdev-rs)
[![crate](https://img.shields.io/crates/v/gpiocdev.svg?color=fc8d62&logo=rust)](https://crates.io/crates/gpiocdev)
![LICENSE](https://img.shields.io/crates/l/gpiocdev.svg)

A Rust library for accessing GPIO lines on Linux platforms using the GPIO character device.

This is the equivalent of [libgpiod](https://git.kernel.org/pub/scm/libs/libgpiod/libgpiod.git/) but in pure Rust.  Unlike the **libgpiod** bindings, this library supports both versions of the GPIO uAPI and supports Rust async.

## Companion Crates

The [gpiocdev-embedded-hal](https://crates.io/crates/gpiocdev-embedded-hal) crate provides
[embedded_hal](https://crates.io/crates/embedded-hal) traits for `Requests`, and provides
a simplified interface that may be useful for basic use cases.

The [gpiocdev-uapi](https://crates.io/crates/gpiocdev-uapi) crate provides a lower level safe wrapper around the GPIO uAPI, while this crate provides a higher level abstraction and unified interface for both uAPI v1 and v2.  You almost certainly want to use a higher level abstraction.

The [gpiocdev-cli](https://crates.io/crates/gpiocdev-cli) crate provides a command line tool, similar to the **libgpiod** tools, for accessing GPIO lines from shell.

The [gpiosim](https://crates.io/crates/gpiosim) crate provides GPIO simulators used to test **gpiocdev**, and potentially any applications that use it.

## Example Usage

```rust
use gpiocdev::Request;
use gpiocdev::line::Value;
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
    let value = req.lone_value()?;
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
    req.set_lone_value(Value::Inactive)
```

Requesting a line by name:

```rust
    let led0 = gpiocdev::find_named_line("LED0").unwrap();
    let req = Request::builder()
        .with_found_line(&led0)
        .as_output(Value::Active)
        .request()?;

    // change value later
    req.set_lone_value(Value::Inactive)
```

Waiting for events on a line:

```rust
    // request the line
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_line(23)
        .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
        .request()?;

    // wait for line edge events
    for event in req.edge_events() {
        println!("{:?}", event?);
    }
```

Multiple lines may be selected in a single request, and then be operated on as a unit.

Getting multiple lines at once:

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

Setting multiple lines at once:

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

Requesting lines with different configurations:

```rust
    // request multiple output lines
    let req = Request::builder()
        .on_chip("/dev/gpiochip0")
        .with_lines(&[17,22])
        .as_output(Value::Active)
        .with_lines(&[18,23])
        .as_input()
        .request()?;
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
        .with_bias(gpiocdev::line::Bias::PullUp)
        .request()?;
    // get the value
    let value = req.lone_value()?;
```

Working examples can be found in the [examples](https://github.com/warthog618/gpiocdev-rs/tree/master/lib/examples) directory.

## ABI Compatibility

The library is compatible with the Linux GPIO uAPI, both v1 and v2, including receiving line edge events.

The gpiocdev API provides a unified abstraction for both uAPI versions, but will return an error if v2 features are attempted to be used on a v1-only system.

uAPI v2 specific features include:

- lines with different configurations in one request
- debouncing edge detection input lines
- edge detection on multiple lines in one request
- sequence numbers on edge events
- reconfiguring edge detection without releasing the request
- selection of source clock for edge events

Compatibility with either uAPI version can be selected via features, with the default being uAPI v2.  If built with both, the library can automatically detect and use the most current available version, so defaulting to v2 and falling back to v1 if that is unavailable.

**gpiocdev** does not use the slower and obsoleted **sysfs** GPIO API.

## Async Compatibility

The majority of the GPIO uAPI is synchronous.  The exceptions are waiting for edge events from [Request](https://docs.rs/gpiocdev/latest/gpiocdev/request/struct.Request.html)s, and info change events from [Chip](https://docs.rs/gpiocdev/latest/gpiocdev/chip/struct.Chip.html)s.
Support for asynchronous wrappers around these are provided through features for the following reactors:

|Reactor|Feature|Module|
|---|---|---|
|tokio|async_tokio|gpiocdev::tokio|
|async-io|async_io|gpiocdev::async_io|

Additionally, Chips and Requests also expose their underlying file descriptor, which may be used directly with an async reactor.  An example of this is the **gpiocdev-cli** [edges](https://github.com/warthog618/gpiocdev-rs/blob/master/cli/src/edges.rs) command, which can asynchronously wait on multiple lines spread across multiple chips using the [mio](https://crates.io/crates/mio) reactor.

With respect to the synchronous uAPI functions, those can generally be considered non-blocking unless the GPIO line is provided by an expander connected to the host processor via a bus such as I2C or SPI.  In such cases, and depending on the application requirements and the async reactor, calls to synchronous functions may need to be made from a separate thread so as not to stall a single-threaded reactor.

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
