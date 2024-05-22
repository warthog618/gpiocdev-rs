Each crate has its own detailed CHANGELOG:

- [cli](cli/CHANGELOG.md)
- [embedded-hal](embedded-hal/CHANGELOG.md)
- [lib](lib/CHANGELOG.md)
- [uapi](uapi/CHANGELOG.md)

## [Unreleased](https://github.com/warthog618/gpiocdev-rs/compare/lib-v0.7.1...HEAD)

## 2024-05-23

- remove dependencies on `nohash-hasher`.
- move some v2::LineConfig functions from lib to uapi.
- fix issue #17 - musl build errors.

### Crates

- [cli v0.5.4](cli/CHANGELOG.md)
- [embedded-hal v0.1.1](embedded-hal/CHANGELOG.md)
- [lib v0.7.1](lib/CHANGELOG.md)
- [uapi v0.6.2](uapi/CHANGELOG.md)

## 2024-02-20

- fix issue #14 - drop `Timestamp` class and `chrono` dependency from lib.
- impl `From` for async wrappers.
- impl `AsRef` for `Request` and `Chip` to simplify usage of async wrappers.
- add embedded-hal crate.

### Crates

- [cli v0.5.3](cli/CHANGELOG.md)
- [embedded-hal v0.1.0](embedded-hal/CHANGELOG.md)
- [lib v0.7.0](lib/CHANGELOG.md)
- [uapi v0.6.1](uapi/CHANGELOG.md)

## 2023-11-22

- add chip path accessors to lib `Request` and `Config`.
- fix clippy 1.75 lints
- support io safety

### Crates

- [lib v0.6.1](lib/CHANGELOG.md)
- [uapi v0.6.0](uapi/CHANGELOG.md)

## 2023-08-11

- fix issue #11 - cli get panics if request fails

### Crates

- [cli v0.5.2](cli/CHANGELOG.md)

## 2023-07-18

- performance improvements.
- event buffer `u64` sizing.

### Crates

- [cli v0.5.1](cli/CHANGELOG.md)
- [lib v0.6.0](lib/CHANGELOG.md)
- [uapi v0.5.0](uapi/CHANGELOG.md)

## 2023-07-04

- fix issue #4 - JSON output for cli.
- fix reading events on 32bit platforms.

### Crates

- [cli v0.5.0](cli/CHANGELOG.md#v0.5.0)
- [lib v0.5.0](lib/CHANGELOG.md#v0.5.0)
- [uapi v0.4.0](uapi/CHANGELOG.md#v0.4.0)

## 2023-05-27

- fix issue #2 - auto-selection of ABI.
- add `platform` command to cli.
- cli commands no longer quote strings by default.

### Crates

- [cli v0.4.2](cli/CHANGELOG.md#v0.4.2)
- [lib v0.4.3](lib/CHANGELOG.md#v0.4.3)
- [uapi v0.3.3](uapi/CHANGELOG.md#v0.3.3)

## 2023-05-21

- clippy lints

### Crates

- [cli v0.4.1](cli/CHANGELOG.md#v0.4.1)
- [lib v0.4.2](lib/CHANGELOG.md#v0.4.2)
- [uapi v0.3.2](uapi/CHANGELOG.md#v0.3.2)

## 2022-11-30

- fix issue #1 - losing sync between `fd` and `AsyncFd`.

### Crates

- [lib v0.4.1](lib/CHANGELOG.md#v0.4.1)

## 2022-11-24

- add `--consumer` option to cli commands.

### Crates

- [cli v0.4.0](cli/CHANGELOG.md#v0.4.0)

## 2022-11-22

- `find_named_lines()` no longer errors if lines are spread across chips.

### Crates

- [lib v0.4.0](lib/CHANGELOG.md#v0.4.0)
- [uapi v0.3.1](uapi/CHANGELOG.md#v0.3.1)

## 2022-10-29

- add Apache-2.0 licensing.
- add async support.

### Crates

- [cli v0.3.0](cli/CHANGELOG.md#v0.3.0)
- [lib v0.3.0](lib/CHANGELOG.md#v0.3.0)
- [uapi v0.3.0](uapi/CHANGELOG.md#v0.3.0)

## 2022-10-23

- extend `lib` API.

### Crates

- [cli v0.2.0](cli/CHANGELOG.md#v0.2.0)
- [lib v0.2.0](lib/CHANGELOG.md#v0.2.0)
- [uapi v0.2.0](uapi/CHANGELOG.md#v0.2.0)

## 2022-10-11

- initial MIT licensed release.

### Crates

- [cli v0.1.0](cli/CHANGELOG.md#v0.1.0)
- [lib v0.1.0](lib/CHANGELOG.md#v0.1.0)
- [uapi v0.1.0](uapi/CHANGELOG.md#v0.1.0)
