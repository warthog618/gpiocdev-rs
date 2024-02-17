// SPDX-FileCopyrightText: 2024 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Asynchronous wrappers for the Tokio reactor.
#[cfg(feature = "async_tokio")]
pub mod tokio;

/// Asynchronous wrappers for the async-io reactor.
#[cfg(feature = "async_io")]
pub mod async_io;
