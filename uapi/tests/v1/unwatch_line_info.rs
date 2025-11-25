// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::common::EVENT_WAIT_TIMEOUT;

#[test]
fn unwatch() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;

    // while unwatched
    assert_eq!(
        unwatch_line_info(&f, offset),
        Err(Error::Os(Errno(libc::EBUSY)))
    );

    // watched
    let info = watch_line_info(&f, offset).expect("watch_line_info should succeed");
    assert_eq!(info.offset, offset);

    assert_eq!(unwatch_line_info(&f, offset), Ok(()));

    let mut hr = HandleRequest {
        num_lines: 1,
        consumer: "unwatch".into(),
        ..Default::default()
    };
    hr.offsets.set(0, offset);

    let l: fs::File = get_line_handle(&f, hr).expect("get_line_handle should succeed");
    assert!(!wait_event(&f, EVENT_WAIT_TIMEOUT).expect("wait_event should succeed"));
    drop(l);
    assert!(!wait_event(&f, EVENT_WAIT_TIMEOUT).expect("wait_event should succeed"));
}

#[test]
fn with_offset_out_of_range() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 4;

    assert_eq!(
        unwatch_line_info(&f, offset),
        Err(Error::Os(Errno(libc::EINVAL)))
    );
}
