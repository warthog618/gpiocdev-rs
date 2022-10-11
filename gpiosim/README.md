# gpiosim

[![github](https://img.shields.io/badge/github-warthog618/gpiocdev--rs-8da0cb.svg?style=for-the-badge&logo=github)](https://github.com/warthog618/gpiocdev-rs)
[![crate](https://img.shields.io/crates/v/gpiosim.svg?style=for-the-badge&color=fc8d62&logo=rust)](https://crates.io/crates/gpiosim)
[![MIT](https://img.shields.io/badge/License-MIT-brightgreen.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)

GPIO chip simulators for testing users of the Linux GPIO uAPI (both v1 and v2).

The simulators are provided by the Linux **gpio-sim** kernel module and require a
recent kernel (v5.19 or later) built with **CONFIG_GPIO_SIM**.

Simulators contain one or more chips, each with a collection of lines being
simulated. Configuring a simulator involves adding **Banks**, representing the
chips, to the simulator, then taking the simulator live.  Once live the **Chip**
exposes lines which may be manipulated to drive the GPIO uAPI from the kernel
side.

The **Builder** is responsible for constructing the **Sim** and taking it live.
For simple tests where all that is needed are lines to be driven on a single
chip, the **Simpleton** provides a simplified interface.

Configuring a simulator involves *configfs*, and manipulating the chips once live
involves *sysfs*, so root permissions are typically required to run a simulator.
