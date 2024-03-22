// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::common::EVENT_WAIT_TIMEOUT;

#[test]
fn watch() {
    use gpiocdev_uapi::v1::{read_event, LineInfoChangeEvent, LineInfoChangeKind};
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;

    let info = watch_line_info(&f, offset).unwrap();
    assert_eq!(info.offset, offset);

    let mut hr = HandleRequest {
        num_lines: 1,
        consumer: "as_input".into(),
        flags: HandleRequestFlags::INPUT,
        ..Default::default()
    };
    hr.offsets.set(0, offset);

    let size = LineInfoChangeEvent::u64_size();
    let mut buf = vec![0_u64; size];
    // request
    let l: fs::File = get_line_handle(&f, hr.clone()).unwrap();
    assert!(wait_event(&f, EVENT_WAIT_TIMEOUT).unwrap());
    assert_eq!(read_event(&f, &mut buf), Ok(size));
    let mut event = LineInfoChangeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.info.offset, offset);
    assert_eq!(event.kind, LineInfoChangeKind::Requested);

    // reconfigure
    let hc = HandleConfig {
        flags: HandleRequestFlags::INPUT,
        ..Default::default()
    };
    set_line_config(&l, hc).unwrap();
    assert!(wait_event(&f, EVENT_WAIT_TIMEOUT).unwrap());
    assert_eq!(read_event(&f, &mut buf), Ok(size));
    event = LineInfoChangeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.info.offset, offset);
    assert_eq!(event.kind, LineInfoChangeKind::Reconfigured);

    // release
    drop(l);
    assert!(wait_event(&f, EVENT_WAIT_TIMEOUT).unwrap());
    assert_eq!(read_event(&f, &mut buf), Ok(size));
    event = LineInfoChangeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.info.offset, offset);
    assert_eq!(event.kind, LineInfoChangeKind::Released);

    // while already watched on this file
    assert_eq!(
        watch_line_info(&f, offset),
        Err(Error::Os(Errno(libc::EBUSY)))
    );

    // request
    let l: fs::File = get_line_handle(&f, hr).unwrap();
    assert!(wait_event(&f, EVENT_WAIT_TIMEOUT).unwrap());
    assert_eq!(read_event(&f, &mut buf), Ok(size));
    event = LineInfoChangeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.info.offset, offset);
    assert_eq!(event.kind, LineInfoChangeKind::Requested);
    drop(l);
}

#[test]
fn with_multiple_watchers() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;

    let info = watch_line_info(&f, offset).unwrap();
    assert_eq!(info.offset, offset);

    let f2 = fs::File::open(s.dev_path()).unwrap();
    let info = watch_line_info(&f2, offset).unwrap();
    assert_eq!(info.offset, offset);
}

#[test]
fn with_offset_out_of_range() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 4;

    assert_eq!(
        watch_line_info(&f, offset),
        Err(Error::Os(Errno(libc::EINVAL)))
    );
}
