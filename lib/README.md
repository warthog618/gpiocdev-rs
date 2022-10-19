# gpiocdev

[![github](https://img.shields.io/badge/github-warthog618/gpiocdev--rs-8da0cb.svg?style=for-the-badge&logo=github)](https://github.com/warthog618/gpiocdev-rs)
[![crate](https://img.shields.io/crates/v/gpiocdev.svg?style=for-the-badge&color=fc8d62&logo=rust)](https://crates.io/crates/gpiocdev)
[![MIT](https://img.shields.io/badge/License-MIT-brightgreen.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)

A Rust library for accessing GPIO lines on Linux platforms using the GPIO character device.

This is the equivalent of [libgpiod](https://git.kernel.org/pub/scm/libs/libgpiod/libgpiod.git/) but in pure Rust.

The gpiocdev crate provides an API to access GPIOs from Rust applications.

## ABI Compatibility

The library is compatible with the Linux GPIO uAPI, both v1 and v2, including receiving line edge events.

The gpiocdev API provides a unified abstraction for both uAPI versions, but will return an error if v2 featuresa are attempted to be used on a v1 system.

uAPI v2 specific features include:

- debouncing input lines
- edge detection on multiple lines in one request
- reconfiguring edge detection without releasing the request

Compatibility with either uAPI version can be selected via features, with the default being uAPI v2.  If built with both, the library will automatically detect and use the most current available version, so defaulting to v2 and falling back to v1 if that is unavailable.

The library makes no use of the deprecated **sysfs** GPIO API.

## Async Compatibility

The majority of the GPIO uAPI is non-blocking and so does not require any async specific treatment.

The exceptions are reading edge events from line requests, and info change events from chips.  Presently these expose the underlying file descriptor, which may be used directly with an async reactor.  An example of this is the **gpiocdev-cli** [edges](https://github.com/warthog618/gpiocdev-rs/blob/master/cli/src/edges.rs) command, which can asynchronously wait on multiple lines spread across multiple chips.

## Example Usage

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
        .as_output(Value.Active)
        .request()?;

    // do something...

    // change value later
    req.set_value(22, Value.Inactive)
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
