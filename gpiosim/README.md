<!--
SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>

SPDX-License-Identifier: CC0-1.0
-->
# gpiosim

[![Build Status](https://img.shields.io/github/workflow/status/warthog618/gpiocdev-rs/Build.svg?logo=github)](https://github.com/warthog618/gpiocdev-rs/actions)
[![github](https://img.shields.io/badge/github-warthog618/gpiocdev--rs-8da0cb.svg?logo=github)](https://github.com/warthog618/gpiocdev-rs)
[![crate](https://img.shields.io/crates/v/gpiosim.svg?color=fc8d62&logo=rust)](https://crates.io/crates/gpiosim)
![LICENSE](https://img.shields.io/crates/l/gpiosim.svg)

A library for creating and controlling GPIO simulators for testing users of
the Linux GPIO uAPI (both v1 and v2).

The simulators are provided by the Linux **gpio-sim** kernel module and require a
recent kernel (v5.19 or later) built with **CONFIG_GPIO_SIM**.

Simulators contain one or more chips, each with a collection of lines being
simulated. A **Builder** is responsible for constructing the **Sim** and taking it live.
Configuring a simulator involves adding **Bank**s, representing the
chip, to the builder, then taking the simulator live.

Once live, the **Chip** exposes lines which may be manipulated to drive the
GPIO uAPI from the kernel side.
For input lines, applying a pull using **Chip** pull methods controls the level
of the simulated line.  For output lines, the **Chip.get_level** method returns
the level the simulated line is being driven to.

For simple tests that only require lines on a single chip, the **Simpleton**
provides a simplified interface.

Configuring a simulator involves *configfs*, and manipulating the chips once live
involves *sysfs*, so root permissions are typically required to run a simulator.

## Example Usage

Creating a simulator with two chips, with 8 and 42 lines respectively, each with
several named lines and a hogged line:

```rust
use gpiosim::{Bank, Direction, Level};

let sim = gpiosim::builder()
    .with_name("some unique name")
    .with_bank(
        Bank::new(8, "left")
            .name(3, "banana")
            .name(5, "apple")
            .hog(2, "hogster", Direction::OutputLow)
        )
    .with_bank(
        Bank::new(42, "right")
            .name(3, "piñata")
            .name(4, "piggly")
            .hog(7, "hogster", Direction::OutputHigh),
    )
    .live()?;

let chips = sim.chips();
let c = &chips[0];
c.set_pull(5, Level::High);
let level = c.get_level(3)?;
```

Use a simpleton to create a single chip simulator with 12 lines, for where multiple chips or
named lines are not required:

```rust
let s = gpiosim::Simpleton::new(12);
let c = s.chip();
c.set_pull(5, Level::High);
let level = c.get_level(3)?;
```

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
