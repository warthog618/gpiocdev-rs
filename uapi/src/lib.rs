// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

//! A thin but safe Rust layer around the Linux GPIO uAPI.

pub(crate) mod common;

pub use common::{
    has_event, wait_event, Error, Name, Result, ValidationError, NAME_LEN_MAX, NUM_LINES_MAX,
};

/// This module implements GPIO ABI v1 which was released in Linux v4.8.
///
/// This ABI version is deprecated.
///
/// ABI v2 adds features and overcomes a number of bugs and limitations present in v1.
/// Use ABI v2 instead where possible.
#[cfg(feature = "uapi_v1")]
pub mod v1;

/// This module implements GPIO ABI v2 which is the current version of the ABI,
/// released in Linux v5.10.
#[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
pub mod v2;
