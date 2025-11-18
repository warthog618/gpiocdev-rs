<a name="unreleased"></a>

## Unreleased

- fix #21 - release builds incorrectly report 0ns debounce period.
- update dependencies.
- update MSRV to 1.68 for quote v1.0.42.
- clippy 1.90 lints.
- stick to C enums instead of Rust enums in uapi structs.

<a name="v0.6.4"></a>

## v0.6.4 - 2025-03-03

- update thiserror dependency

<a name="v0.6.3"></a>

## v0.6.3 - 2024-10-03

- support android build

<a name="v0.6.2"></a>

## v0.6.2 - 2024-05-23

- add methods to add attributes to v2::LineConfig.
- fix musl build errors and warnings

<a name="v0.6.1"></a>

## v0.6.1 - 2024-02-20

- fix and extend benchmarks

<a name="v0.6.0"></a>

## v0.6.0 - 2023-11-22

- change API from `RawFd` to `&File` for io safety.

<a name="v0.5.0"></a>

## v0.5.0 - 2023-07-18

- use `u64` rather than `Bitmap` for v2 bitmaps.
- add benches.

<a name="v0.4.0"></a>

## v0.4.0 - 2023-07-04

- update MSRV to 1.62.
- drop dependency on `errno`.
- fix reading events on 32bit platforms.

<a name="v0.3.3"></a>

## v0.3.3 - 2023-05-27

- update `bitflags` dependency.

<a name="v0.3.2"></a>

## v0.3.2 - 2023-05-21

- fix clippy lints.

<a name="v0.3.1"></a>

## v0.3.1 - 2022-11-22

- better handle truncated UTF-8 strings returned by kernel.

<a name="v0.3.0"></a>

## v0.3.0 - 2022-10-29

- add option of Apache-2.0 licensing.
- rename `from_buf()` functions to `from_slice()`.
- check for under-read when reading events from slice.

<a name="v0.2.0"></a>

## v0.2.0 - 2022-10-23

- make `Errors` cloneable.
- generate docs for all features.

<a name="v0.1.0"></a>

## v0.1.0 - 2022-10-11

- initial release under MIT license.
