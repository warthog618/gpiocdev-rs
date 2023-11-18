// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::common::{wait_propagation_delay, EVENT_WAIT_TIMEOUT};

#[test]
fn as_is() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let er = EventRequest {
        consumer: "as_is".into(),
        offset,
        // no handleflags set - INPUT is implied
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    let l = get_line_event(&f, er.clone()).unwrap();
    let info = get_line_info(&f, offset).unwrap();
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
    let l = get_line_handle(&f, hr).unwrap();
    drop(l);

    // switched to input
    let l = get_line_event(&f, er).unwrap();
    let info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, LineInfoFlags::USED);
    // v1 does not report edge flags in the info, so nothing more to check
    drop(l);
}

#[test]
fn as_input() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let er = EventRequest {
        consumer: "as_input".into(),
        offset,
        handleflags: HandleRequestFlags::INPUT,
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    let l = get_line_event(&f, er).unwrap();
    let info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_input");
    assert_eq!(info.flags, LineInfoFlags::USED);
    // v1 does not report edge flags in the info, so nothing more to check
    drop(l);
}

#[test]
fn as_output() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let er = EventRequest {
        consumer: "as_output".into(),
        offset,
        handleflags: HandleRequestFlags::OUTPUT,
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    assert_eq!(
        get_line_event(&f, er).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn with_both_edges() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let er = EventRequest {
        offset,
        consumer: "with_both_edges".into(),
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    let l = get_line_event(&f, er).unwrap();

    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();

    let mut buf = vec![0_u64; LineEdgeEvent::u64_size()];
    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    let mut event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge);

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge);

    assert!(!wait_event(&l, EVENT_WAIT_TIMEOUT).unwrap());

    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge);

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge);

    drop(l);
}

#[test]
fn with_rising_edge() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let er = EventRequest {
        offset,
        consumer: "with_rising_edge".into(),
        eventflags: EventRequestFlags::RISING_EDGE,
        ..Default::default()
    };

    let l = get_line_event(&f, er).unwrap();

    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();
    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();

    let mut buf = vec![0_u64; LineEdgeEvent::u64_size()];
    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    let mut event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge);

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge);

    assert!(!wait_event(&l, EVENT_WAIT_TIMEOUT).unwrap());

    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge);

    assert!(!has_event(&l).unwrap());

    drop(l);
}

#[test]
fn with_falling_edge() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let er = EventRequest {
        offset,
        consumer: "with_falling_edge".into(),
        eventflags: EventRequestFlags::FALLING_EDGE,
        ..Default::default()
    };

    let l = get_line_event(&f, er).unwrap();

    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();
    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();

    let mut buf = vec![0_u64; LineEdgeEvent::u64_size()];
    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    let mut event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge);

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge);

    assert!(!wait_event(&l, EVENT_WAIT_TIMEOUT).unwrap());

    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge);

    assert!(!has_event(&l).unwrap());

    drop(l);
}

#[test]
fn with_offset_out_of_range() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 4;
    let er = EventRequest {
        offset,
        consumer: "with_offset_out_of_range".into(),
        handleflags: HandleRequestFlags::INPUT,
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };
    assert_eq!(
        get_line_event(&f, er).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn while_busy() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let er = EventRequest {
        consumer: "while_busy".into(),
        offset,
        handleflags: HandleRequestFlags::INPUT,
        eventflags: EventRequestFlags::BOTH_EDGES,
        ..Default::default()
    };

    let l: fs::File = get_line_event(&f, er.clone()).unwrap();

    assert_eq!(
        get_line_event(&f, er).unwrap_err(),
        Error::Os(Errno(libc::EBUSY))
    );
    drop(l);
}

#[test]
fn with_multiple_bias_flags() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
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
        get_line_event(&f, er).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    er = er_base.clone();
    er.handleflags |= HandleRequestFlags::BIAS_PULL_UP;
    er.handleflags |= HandleRequestFlags::BIAS_DISABLED;
    assert_eq!(
        get_line_event(&f, er).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    er = er_base;
    er.handleflags |= HandleRequestFlags::BIAS_PULL_DOWN;
    er.handleflags |= HandleRequestFlags::BIAS_DISABLED;
    assert_eq!(
        get_line_event(&f, er).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn with_drive_flags() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
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
        get_line_event(&f, er).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    er = er_base;
    er.handleflags |= HandleRequestFlags::OPEN_SOURCE;
    assert_eq!(
        get_line_event(&f, er).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}
