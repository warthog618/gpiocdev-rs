<a name="unreleased"></a>
## Unreleased

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
 - drop dependncy on `errno`.
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
 - fix `Builder::weith_found_lines()` line selection.

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

