<a name="unreleased"></a>

## Unreleased

- replace panicky unwrap variants with expect variants.

<a name="v0.8.0"></a>

## v0.8.0 - 2025-11-19

- update criterion dependency to 0.7.
- clippy 1.90 lints.
- update  MSRV to 1.68 for quote v1.0.42.
- uapi using C enums instead of Rust enums.
- fix race conditions in tests.

<a name="v0.7.3"></a>

## v0.7.3 - 2025-03-03

- update thiserror dependency.
- make tests more robust.
- update MSRV to 1.64 for tokio example.
- fix clippy lints.

<a name="v0.7.2"></a>

## v0.7.2 - 2024-10-03

- add documentation to builder for complex configurations.
- add Request.lone_value() and set_lone_value().
- support android build.

<a name="v0.7.1"></a>

## v0.7.1 - 2024-05-23

- remove dependency on `nohash-hasher`.
- move adding attributes to v2::LineConfig to uapi.
- use local var println formatting.


<a name="v0.7.0"></a>

## v0.7.0 - 2024-02-20

- assume `/dev/gpiochipXX` are valid gpiochips without explicit checking.
- fix issue #14 - drop Timestamp class and dependency on `chrono` - was only for **CLOCK_REALTIME** helper.
- impl From for async wrappers - both ways.
- move input/output sanitization from `request::Config` to `line::Config`.
- impl `AsRef` for `Request` and `Chip` to simplify usage of async wrappers.
- add reconfigure bench.

<a name="v0.6.1"></a>

## v0.6.1 - 2023-11-22

- rework README.
- add chip path accessors to Request and Config.
- fix clippy 1.75 lints
- add AsFd support to `Chip` and `Request` (bumps MSRV to 1.63).
- update to gpiocdev-uapi 0.6 (io safety)
- update to async-io 2.2

<a name="v0.6.0"></a>

## v0.6.0 - 2023-07-18

- replace `Bitmaps` with `u64`.
- functions to read edge events to and from slices all use `u64` sizes instead of byte sizes.
- optimise `Values` to improve set/get performance.
- refactor benches.
- refactor tests.
- extend test timeouts for slower platforms (e.g. Pi Zero W).

<a name="v0.5.0"></a>

## v0.5.0 - 2023-07-04

- update MSRV to 1.62.
- fix reading events on 32bit platforms.
- drop dependency on `errno`.
- add `serde` feature support.
- re-export `async_io` and `tokio` to remove `r#async` layer.
- re-export `chip::Chip` and `request::Request` at top level to avoid stutter.
- don't leak optional dependencies as features.
- refactor `line` and `request`.
- add benches.
- rework examples.

<a name="v0.4.3"></a>

## v0.4.3 - 2023-05-27

- fix issue #2 - auto-selection of uAPI version.
- `Builder::request()` now requires a `&mut self`.
- add `Builder::as_is()` option.
- rename `AbiSupportKind` variants to improve clarity.
- human sort `chips()`.
- fix `Builder::with_found_lines()` line selection.

<a name="v0.4.2"></a>

## v0.4.2 - 2023-05-21

- document lifetime of `Output` settings.
- add `tokio_ticking_watcher` example.
- fix clippy lints.

<a name="v0.4.1"></a>

## v0.4.1 - 2022-11-30

- fix issue #1 - losing sync between fd and AsyncFd.

<a name="v0.4.0"></a>

## v0.4.0 - 2022-11-22

- set MSRV at 1.60.
- add doc examples for async wrappers.
- `find_named_lines()` no longer errors if lines are spread across chips.

<a name="v0.3.0"></a>

## v0.3.0 - 2022-10-29

- add option of Apache-2.0 licensing.
- add async support for both `tokio` and `async-io` compatible reactors.

<a name="v0.2.0"></a>

## v0.2.0 - 2022-10-23

- add `Offset` type instead of `u32`.
- add `Chip::from_name()`.
- change `Chip::find_line()` to `find_line_info()` as full info is available.
- add `Chip::line_info_iterator()`.
- add doc examples.
- add `lines()` iterator.
- add `find_named_line()` and `find_named_lines()`.
- add `Builder::with_found_line()` option.
- rename `_from_buf()` functions to `_from_slice()`.
- sort `chip::chips()`, so return `Vec` rather than `Iterator`.

<a name="v0.1.0"></a>

## v0.1.0 - 2022-10-11

- initial release under MIT license.
