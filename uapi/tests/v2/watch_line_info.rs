// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::common::EVENT_WAIT_TIMEOUT;

#[test]
fn watch() {
    use gpiocdev_uapi::v2::{read_event, LineConfig, LineInfoChangeEvent, LineInfoChangeKind};
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;

    // watched
    let info = watch_line_info(&f, offset).expect("watch_line_info should succeed");
    assert_eq!(info.offset, offset);

    let mut lr = LineRequest {
        num_lines: 1,
        consumer: "watch".into(),
        ..Default::default()
    };
    lr.offsets.set(0, offset);

    let size = LineInfoChangeEvent::u64_size();
    let mut buf = vec![0_u64; size];
    // request
    let l: fs::File = get_line(&f, lr.clone()).expect("get_line should succeed");
    assert!(wait_event(&f, EVENT_WAIT_TIMEOUT).expect("wait_event should succeed"));
    assert_eq!(read_event(&f, &mut buf), Ok(size));
    let mut event = LineInfoChangeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.info.offset, offset);
    assert_eq!(event.kind, LineInfoChangeKind::Requested as u32);

    // reconfigure
    let lc = LineConfig {
        flags: LineFlags::INPUT,
        ..Default::default()
    };
    set_line_config(&l, lc).expect("set_line_config should succeed");
    assert!(wait_event(&f, EVENT_WAIT_TIMEOUT).expect("wait_event should succeed"));
    assert_eq!(read_event(&f, &mut buf), Ok(size));
    event = LineInfoChangeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.info.offset, offset);
    assert_eq!(event.kind, LineInfoChangeKind::Reconfigured as u32);

    // release
    drop(l);
    assert!(wait_event(&f, EVENT_WAIT_TIMEOUT).expect("wait_event should succeed"));
    assert_eq!(read_event(&f, &mut buf), Ok(size));
    event = LineInfoChangeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.info.offset, offset);
    assert_eq!(event.kind, LineInfoChangeKind::Released as u32);

    // while already watched on this fd
    assert_eq!(
        watch_line_info(&f, offset),
        Err(Error::Os(Errno(libc::EBUSY)))
    );

    // request
    let l: fs::File = get_line(&f, lr).expect("get_line should succeed");
    assert!(wait_event(&f, EVENT_WAIT_TIMEOUT).expect("wait_event should succeed"));
    assert_eq!(read_event(&f, &mut buf), Ok(size));
    event = LineInfoChangeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.info.offset, offset);
    assert_eq!(event.kind, LineInfoChangeKind::Requested as u32);
    drop(l);
}

#[test]
fn with_multiple_watchers() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;

    let info = watch_line_info(&f, offset).expect("watch_line_info should succeed");
    assert_eq!(info.offset, offset);

    let f2 = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let info = watch_line_info(&f2, offset).expect("watch_line_info should succeed");
    assert_eq!(info.offset, offset);
}

#[test]
fn with_offset_out_of_range() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 4;

    assert_eq!(
        watch_line_info(&f, offset),
        Err(Error::Os(Errno(libc::EINVAL)))
    );
}
