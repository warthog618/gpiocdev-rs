// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::common::EVENT_WAIT_TIMEOUT;

#[test]
fn unwatch() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;

    // while unwatched
    assert_eq!(
        unwatch_line_info(&f, offset),
        Err(Error::Os(Errno(libc::EBUSY)))
    );

    // watched
    let info = watch_line_info(&f, offset).unwrap();
    assert_eq!(info.offset, offset);

    assert_eq!(unwatch_line_info(&f, offset), Ok(()));

    let mut lr = LineRequest {
        num_lines: 1,
        consumer: "unwatch".into(),
        ..Default::default()
    };
    lr.offsets.set(0, offset);

    let l: fs::File = get_line(&f, lr).unwrap();
    assert!(!wait_event(&f, EVENT_WAIT_TIMEOUT).unwrap());
    drop(l);
    assert!(!wait_event(&f, EVENT_WAIT_TIMEOUT).unwrap());
}

#[test]
fn with_offset_out_of_range() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 4;

    assert_eq!(
        unwatch_line_info(&f, offset),
        Err(Error::Os(Errno(libc::EINVAL)))
    );
}
