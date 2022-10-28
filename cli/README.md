<!--
SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>

SPDX-License-Identifier: CC0-1.0
-->
# gpiocdev-cli

[![Build Status](https://img.shields.io/github/workflow/status/warthog618/gpiocdev-rs/Build.svg?logo=github)](https://github.com/warthog618/gpiocdev-rs/actions)
[![github](https://img.shields.io/badge/github-warthog618/gpiocdev--rs-8da0cb.svg?logo=github)](https://github.com/warthog618/gpiocdev-rs)
[![crate](https://img.shields.io/crates/v/gpiocdev-cli.svg?color=fc8d62&logo=rust)](https://crates.io/crates/gpiocdev-cli)
![LICENSE](https://img.shields.io/crates/l/gpiocdev-cli.svg)

A command line tool to access and control GPIO lines on Linux.

This is equivalent to the tools provided by [libgpiod](https://git.kernel.org/pub/scm/libs/libgpiod/libgpiod.git/), combined into a single tool.

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

Refer to the help for each subcommand for more details.

## Example Usage

### chip

```shell
$ gpiocdev chip
gpiochip0 [pinctrl-bcm2711] (58 lines)
gpiochip1 [raspberrypi-exp-gpio] (8 lines)
```

### line

```shell
$ gpiocdev line -c 1
gpiochip1 - 8 lines:
	line   0:	BT_ON           	output
	line   1:	WL_ON           	output
	line   2:	PWR_LED_OFF     	output active-low consumer=led1
	line   3:	GLOBAL_RESET    	output
	line   4:	VDD_SD_IO_SEL   	output consumer=vdd-sd-io
	line   5:	CAM_GPIO        	output consumer=cam1_regulator
	line   6:	SD_PWR_ON       	output consumer=sd_vcc_reg
	line   7:	SD_OC_N         	input

$ gpiocdev line RXD0 GPIO22 WL_ON
gpiochip0 22	GPIO22          	input
gpiochip0 33	RXD0            	input
gpiochip1 1	WL_ON           	output
```

### get

```shell
$ gpiocdev get RXD0 GPIO22
RXD0=active GPIO22=inactive

$ gpiocdev get --numeric RXD0 GPIO22
1 0
```

### set

```shell
$ gpiocdev set GPIO22=active

$ gpiocdev set GPIO17=1
```

Interactive mode:

```shell
$ gpiocdev set -i GPIO23=1 GPIO24=0
gpiocdev-set> get
GPIO23=active GPIO24=inactive
gpiocdev-set> toggle 
gpiocdev-set> get
GPIO23=inactive GPIO24=active
gpiocdev-set> toggle GPIO23
gpiocdev-set> get
GPIO23=active GPIO24=active
gpiocdev-set> set GPIO24=0
gpiocdev-set> get
GPIO23=active GPIO24=inactive
gpiocdev-set> exit
```

### edges

```shell
$ gpiocdev edges GPIO22
264.429997058	rising	GPIO22
270.948800095	falling	GPIO22
279.482197087	rising	GPIO22

$ gpiocdev edges --localtime GPIO22
2022-10-10T08:01:59.078908766	falling	GPIO22
2022-10-10T08:01:59.810733200	rising	GPIO22
2022-10-10T08:02:00.497461672	falling	GPIO22
```

### notify

```shell
$ gpiocdev notify GPIO23
390.467444514	requested	GPIO23
390.467655548	released	GPIO23
391.701420722	requested	GPIO23
391.701624886	released	GPIO23

$ gpiocdev notify --localtime GPIO23
2022-10-10T08:03:35.365989261	requested	GPIO23
2022-10-10T08:03:35.366200648	released	GPIO23
2022-10-10T08:03:35.884780936	requested	GPIO23
2022-10-10T08:03:35.884881408	released	GPIO23
```

## ABI compatibility

The cli supports both GPIO uAPI v1 and v2.

Options that require v2, such as debounce, will fail if applied to a kernel
that only supports v1.

Either or both uAPI versions can be supported in the one executable.
By default both are supported, with the most recent ABI supported by the
kernel being used.

## libgpiod compatibility

The tools are plug compatible with the proposed tools for **libgpiod v2**, though
unlike those can also work with both GPIO uAPI v1 and v2.

The subcommands map to libgpiod tools as follows:
| Subcommand | libgpiod Tool | Alias |
|------------|------|-------|
| chip | gpiodetect | detect |
| edges | gpiomon | mon |
| get | gpioget | - |
| line | gpioinfo | info |
| notify | gpiowatch | watch |
|set | gpioset | - |

The subcommand aliases provide an alternative drawn from the libgpiod tool
name, minus the gpio prefix. e.g.

```shell
gpiocdev detect
```

is equivalent to

```shell
gpiocdev chip
```

The libgpiod v1 **gpiofind** tool is obsoleted by the **line** subcommand.

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
