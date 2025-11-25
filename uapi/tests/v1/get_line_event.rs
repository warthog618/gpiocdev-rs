// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::common::{wait_propagation_delay, EVENT_WAIT_TIMEOUT};

#[test]
fn as_is() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;
    let er = EventRequest {
        consumer: "as_is".into(),
        offset,
        // no handleflags set - INPUT is implied
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    let l = get_line_event(&f, er.clone()).expect("get_line_event should succeed");
    let info = get_line_info(&f, offset).expect("get_line_info should succeed");
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, LineInfoFlags::USED);
    // v1 does not report edge flags in the info, so nothing more to check
    drop(l);

    // force line to output
    let mut hr = HandleRequest {
        num_lines: 1,
        consumer: "as_is".into(),
        flags: HandleRequestFlags::OUTPUT,
        ..Default::default()
    };
    hr.offsets.set(0, offset);
    let l = get_line_handle(&f, hr).expect("get_line_handle should succeed");
    drop(l);

    // switched to input
    let l = get_line_event(&f, er).expect("get_line_event should succeed");
    let info = get_line_info(&f, offset).expect("get_line_info should succeed");
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, LineInfoFlags::USED);
    // v1 does not report edge flags in the info, so nothing more to check
    drop(l);
}

#[test]
fn as_input() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;
    let er = EventRequest {
        consumer: "as_input".into(),
        offset,
        handleflags: HandleRequestFlags::INPUT,
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    let l = get_line_event(&f, er).expect("get_line_event should succeed");
    let info = get_line_info(&f, offset).expect("get_line_info should succeed");
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_input");
    assert_eq!(info.flags, LineInfoFlags::USED);
    // v1 does not report edge flags in the info, so nothing more to check
    drop(l);
}

#[test]
fn as_output() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;
    let er = EventRequest {
        consumer: "as_output".into(),
        offset,
        handleflags: HandleRequestFlags::OUTPUT,
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    assert_eq!(
        get_line_event(&f, er).expect_err("get_line_event should fail"),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn with_both_edges() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;
    let er = EventRequest {
        offset,
        consumer: "with_both_edges".into(),
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    let l = get_line_event(&f, er).expect("get_line_event should succeed");

    s.pullup(offset).expect("pullup should succeed");
    wait_propagation_delay();
    s.pulldown(offset).expect("pulldown should succeed");
    wait_propagation_delay();

    let mut buf = vec![0_u64; LineEdgeEvent::u64_size()];
    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    let mut event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge as u32);

    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge as u32);

    assert!(!wait_event(&l, EVENT_WAIT_TIMEOUT).expect("wait_event should succeed"));

    s.pullup(offset).expect("pullup should succeed");
    wait_propagation_delay();
    s.pulldown(offset).expect("pulldown should succeed");
    wait_propagation_delay();

    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge as u32);

    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge as u32);

    drop(l);
}

#[test]
fn with_rising_edge() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;
    let er = EventRequest {
        offset,
        consumer: "with_rising_edge".into(),
        eventflags: EventRequestFlags::RISING_EDGE,
        ..Default::default()
    };

    let l = get_line_event(&f, er).expect("get_line_event should succeed");

    s.pullup(offset).expect("pullup should succeed");
    wait_propagation_delay();
    s.pulldown(offset).expect("pulldown should succeed");
    wait_propagation_delay();
    s.pullup(offset).expect("pullup should succeed");
    wait_propagation_delay();
    s.pulldown(offset).expect("pulldown should succeed");
    wait_propagation_delay();

    let mut buf = vec![0_u64; LineEdgeEvent::u64_size()];
    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    let mut event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge as u32);

    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge as u32);

    assert!(!wait_event(&l, EVENT_WAIT_TIMEOUT).expect("wait_event should succeed"));

    s.pullup(offset).expect("pullup should succeed");
    wait_propagation_delay();
    s.pulldown(offset).expect("pulldown should succeed");
    wait_propagation_delay();

    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge as u32);

    assert!(!has_event(&l).expect("has_event should succeed"));

    drop(l);
}

#[test]
fn with_falling_edge() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;
    let er = EventRequest {
        offset,
        consumer: "with_falling_edge".into(),
        eventflags: EventRequestFlags::FALLING_EDGE,
        ..Default::default()
    };

    let l = get_line_event(&f, er).expect("get_line_event should succeed");

    s.pullup(offset).expect("pullup should succeed");
    wait_propagation_delay();
    s.pulldown(offset).expect("pulldown should succeed");
    wait_propagation_delay();
    s.pullup(offset).expect("pullup should succeed");
    wait_propagation_delay();
    s.pulldown(offset).expect("pulldown should succeed");
    wait_propagation_delay();

    let mut buf = vec![0_u64; LineEdgeEvent::u64_size()];
    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    let mut event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge as u32);

    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge as u32);

    assert!(!wait_event(&l, EVENT_WAIT_TIMEOUT).expect("wait_event should succeed"));

    s.pullup(offset).expect("pullup should succeed");
    wait_propagation_delay();
    s.pulldown(offset).expect("pulldown should succeed");
    wait_propagation_delay();

    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge as u32);

    assert!(!has_event(&l).expect("has_event should succeed"));

    drop(l);
}

#[test]
fn without_consumer() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;
    let er = EventRequest {
        offset,
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    let l = get_line_event(&f, er).expect("get_line_event should succeed");

    s.pullup(offset).expect("pullup should succeed");
    wait_propagation_delay();
    s.pulldown(offset).expect("pulldown should succeed");
    wait_propagation_delay();

    let mut buf = vec![0_u64; LineEdgeEvent::u64_size()];
    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    let mut event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge as u32);

    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge as u32);

    assert!(!wait_event(&l, EVENT_WAIT_TIMEOUT).expect("wait_event should succeed"));

    s.pullup(offset).expect("pullup should succeed");
    wait_propagation_delay();
    s.pulldown(offset).expect("pulldown should succeed");
    wait_propagation_delay();

    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge as u32);

    assert!(has_event(&l).expect("has_event should succeed"));
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge as u32);

    drop(l);
}

#[test]
fn with_offset_out_of_range() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 4;
    let er = EventRequest {
        offset,
        consumer: "with_offset_out_of_range".into(),
        handleflags: HandleRequestFlags::INPUT,
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };
    assert_eq!(
        get_line_event(&f, er).expect_err("get_line_event should fail"),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn while_busy() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;
    let er = EventRequest {
        consumer: "while_busy".into(),
        offset,
        handleflags: HandleRequestFlags::INPUT,
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    let l: fs::File = get_line_event(&f, er.clone()).expect("get_line_event should succeed");

    assert_eq!(
        get_line_event(&f, er).expect_err("get_line_event should fail"),
        Error::Os(Errno(libc::EBUSY))
    );
    drop(l);
}

#[test]
fn with_multiple_bias_flags() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;
    let er_base = EventRequest {
        consumer: "with_multiple_bias_flags".into(),
        offset,
        handleflags: HandleRequestFlags::INPUT,
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    let mut er = er_base.clone();
    er.handleflags |= HandleRequestFlags::BIAS_PULL_UP;
    er.handleflags |= HandleRequestFlags::BIAS_PULL_DOWN;
    assert_eq!(
        get_line_event(&f, er).expect_err("get_line_event should fail"),
        Error::Os(Errno(libc::EINVAL))
    );

    er = er_base.clone();
    er.handleflags |= HandleRequestFlags::BIAS_PULL_UP;
    er.handleflags |= HandleRequestFlags::BIAS_DISABLED;
    assert_eq!(
        get_line_event(&f, er).expect_err("get_line_event should fail"),
        Error::Os(Errno(libc::EINVAL))
    );

    er = er_base;
    er.handleflags |= HandleRequestFlags::BIAS_PULL_DOWN;
    er.handleflags |= HandleRequestFlags::BIAS_DISABLED;
    assert_eq!(
        get_line_event(&f, er).expect_err("get_line_event should fail"),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn with_drive_flags() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;
    let er_base = EventRequest {
        consumer: "with_drive_flags".into(),
        offset,
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    let mut er = er_base.clone();
    er.handleflags |= HandleRequestFlags::OPEN_DRAIN;
    assert_eq!(
        get_line_event(&f, er).expect_err("get_line_event should fail"),
        Error::Os(Errno(libc::EINVAL))
    );

    er = er_base;
    er.handleflags |= HandleRequestFlags::OPEN_SOURCE;
    assert_eq!(
        get_line_event(&f, er).expect_err("get_line_event should fail"),
        Error::Os(Errno(libc::EINVAL))
    );
}
