# gpiod-rs
A Rust library for accessing GPIO lines on Linux platforms using the GPIO character device.

This is the equivalent of [libgpiod](https://git.kernel.org/pub/scm/libs/libgpiod/libgpiod.git/) and my Go [gpiod](https://github.com/warthog618/gpiod) library, but in pure Rust.

The crates are still a work in progress.  They are functionally complete, but have only been lightly tested on a couple of platforms.


## gpiodctl

gpiodctl provides a command line tool to access and control GPIO lines.

This is equivalent to the tools provided by libgpiod, combined into a single tool.

```
gpiodctl 0.1.0
A utility to control GPIO lines on Linux GPIO character devices.

USAGE:
    gpiodctl [FLAGS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Provide more detailed error messages

SUBCOMMANDS:
    detect     Get info for the GPIO chips present on the system
    find       Find a line by name
    get        Read the values of a set of lines on a chip
    help       Prints this message or the help of the given subcommand(s)
    info       Get info for the lines on a chip
    monitor    Monitor changes in line values
    set        Set the value of a set of lines on a chip
    watch      Watch changes to line request and configuration state
```

## gpiod

The [gpiod](https://warthog618.github.io/gpiod-rs/gpiod/) crate provides an API for accessing and controlling GPIO lines from Rust applications.

A good starting point is the [gpiod::request::Builder](https://warthog618.github.io/gpiod-rs/gpiod/request/struct.Builder.html).

## uapi

The [uapi](https://warthog618.github.io/gpiod-rs/gpiod_uapi/) crate provides a minimal safe Rust wrapper around the character device ioctls.

The gpiod crate provides a higher level abstraction, so you should use that unless you have some particular interest in calling the ioctls directly.


