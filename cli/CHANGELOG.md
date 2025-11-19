<a name="unreleased"></a>

## Unreleased

<a name="v0.6.0"></a>

## v0.6.0 - 2025-11-19

- fix #21 - release builds incorrectly report 0ns debounce period.
- update dependencies.
- update MSRV to 1.88 for home v0.5.12.
- clippy 1.90 lints.

<a name="v0.5.5"></a>

## v0.5.5 - 2025-03-03

- fix #18 - set doesn't block using ABI v1.
- update mio, rustyline and thiserror dependencies.
- bump MSRV to 1.81 due to home@0.5.11.
- fix clippy lints.

<a name="v0.5.4"></a>

## v0.5.4 - 2024-05-23

- remove dependency on nohash-hasher.
- use local var println formatting.
- replace now deprecated chrono function calls.
- improve help for set --interactive get.

<a name="v0.5.3"></a>

## v0.5.3 - 2024-02-20

- fix clippy 1.75 ints.
- update dependencies.

<a name="v0.5.2"></a>

## v0.5.2 - 2023-08-11

- fix issue #11 - get panics if request fails.

<a name="v0.5.1"></a>

## v0.5.1 - 2023-07-18

- update `gpiocdev` dependency.

<a name="v0.5.0"></a>

## v0.5.0 - 2023-07-04

- update MSRV to 1.64.
- fix issue #4 - add `--json` option to output in JSON.
- fix formatting raw time nanoseconds - was dropping leading zeroes.

<a name="v0.4.2"></a>

## v0.4.2 - 2023-05-27

- add `platform` command.
- update `rustyline` and `daemonize` dependencies.
- expose `--abi-version` option which was previously hidden (and called `--abiv`).
- commands no longer quote line names and consumers by default.  Use `--quoted` to restore old behaviour.

<a name="v0.4.1"></a>

## v0.4.1 - 2023-05-21

- add `gpiocdevd.sh` example.
- fix clippy lints.

<a name="v0.4.0"></a>

## v0.4.0 - 2022-11-24

- `line` quotes names and consumers by default.
- add `--consumer` option to `get`.
- add `-C` short form of `--consumer` to `edges`, `get` and `set`.
- `set --interactive` tab completion now supports quoted line names.

<a name="v0.3.0"></a>

## v0.3.0 - 2022-10-29

- add option of Apache-2.0 licensing.
- add `uapi_v1` and `uapi_v2` features to allow building for a specific uAPI version.

<a name="v0.2.0"></a>

## v0.2.0 - 2022-10-23

- add `--consumer` for edges.
- change default consumer to `gpiocdev-<cmd>`.

<a name="v0.1.0"></a>

## v0.1.0 - 2022-10-11

- initial release under MIT license.
