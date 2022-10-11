# gpiocdev-rs

A Rust library for accessing GPIO lines on Linux platforms using the GPIO character device.

This is the equivalent of [libgpiod](https://git.kernel.org/pub/scm/libs/libgpiod/libgpiod.git/), but in pure Rust.

## gpiocdev

The [gpiocdev](https://warthog618.github.io/gpiocdev-rs/gpiocdev/) crate provides an API for accessing and controlling GPIO lines from Rust applications.

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

A good starting point to learn more is the [gpiocdev::request::Request](https://warthog618.github.io/gpiocdev-rs/gpiocdev/request/struct.Request.html).

## gpiocdev-cli

The gpiocdev-cli crate provides a command line tool to access and control GPIO lines.

This is equivalent to the tools provided by libgpiod, combined into a single tool.

```text
$ gpiocdev -h
A utility to control GPIO lines on Linux using GPIO character devices.

Usage: gpiocdev [OPTIONS] <COMMAND>

Commands:
  chip    Get info about GPIO chips
  edges   Monitor lines for edge events
  get     Read the values of lines
  line    Get information for lines
  notify  Monitor lines for requests and changes to configuration state
  set     Set the value of lines
  help    Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose  Provide more detailed error messages
  -h, --help     Print help information
  -V, --version  Print version information
```

## gpiocdev-uapi

The [gpiocdev-uapi](https://warthog618.github.io/gpiocdev-rs/gpiocdev_uapi/) crate provides a minimal safe Rust wrapper around the character device ioctls.

The gpiocdev crate provides a higher level abstraction, so you should use that unless you have some particular interest in calling the ioctls directly.

## gpiosim

The [gpiosim](https://warthog618.github.io/gpiocdev-rs/gpiosim/) crate provides GPIO chip simulators for testing gpiocdev, or apps that use it.
