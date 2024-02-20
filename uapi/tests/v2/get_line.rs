// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::common::{wait_propagation_delay, EVENT_WAIT_TIMEOUT};

use super::*;

#[test]
fn as_is() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr_base = LineRequest {
        num_lines: 1,
        consumer: "as_is".into(),
        ..Default::default()
    };
    lr_base.offsets.set(0, offset);
    let mut xflags = LineFlags::USED | LineFlags::INPUT;

    // input

    // single line
    let mut lr = lr_base.clone();
    let mut l = get_line(&f, lr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, xflags);
    drop(l);

    // multiple lines
    lr = lr_base.clone();
    lr.num_lines = 3;
    lr.offsets.set(1, 0);
    lr.offsets.set(2, 3);
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, xflags);
    info = get_line_info(&f, 0).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, xflags);
    info = get_line_info(&f, 3).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, xflags);
    drop(l);

    // force lines to output
    lr = lr_base.clone();
    lr.num_lines = 3;
    lr.offsets.set(1, 0);
    lr.offsets.set(2, 3);
    lr.config.flags = LineFlags::OUTPUT;
    l = get_line(&f, lr).unwrap();
    drop(l);

    // output
    xflags = LineFlags::USED | LineFlags::OUTPUT;

    // single line
    let mut lr = lr_base.clone();
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, xflags);
    drop(l);

    // multiple lines
    lr = lr_base.clone();
    lr.num_lines = 3;
    lr.offsets.set(1, 0);
    lr.offsets.set(2, 3);
    let l = get_line(&f, lr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, xflags);
    info = get_line_info(&f, 0).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, xflags);
    info = get_line_info(&f, 3).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, xflags);
    drop(l);
}

#[test]
fn as_input() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr_base = LineRequest {
        num_lines: 1,
        consumer: "as_input".into(),
        config: LineConfig {
            flags: LineFlags::INPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    lr_base.offsets.set(0, offset);
    let xflags = LineFlags::USED | LineFlags::INPUT;

    // single line
    let mut lr = lr_base.clone();
    let mut l = get_line(&f, lr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_input");
    assert_eq!(info.flags, xflags);
    drop(l);

    // multiple lines
    lr = lr_base.clone();
    lr.num_lines = 3;
    lr.offsets.set(1, 0);
    lr.offsets.set(2, 3);
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_input");
    assert_eq!(info.flags, xflags);
    info = get_line_info(&f, 0).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_input");
    assert_eq!(info.flags, xflags);
    info = get_line_info(&f, 3).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_input");
    assert_eq!(info.flags, xflags);
    drop(l);
}

#[test]
fn as_output() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr_base = LineRequest {
        num_lines: 1,
        consumer: "as_output".into(),
        config: LineConfig {
            flags: LineFlags::OUTPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    lr_base.offsets.set(0, offset);
    let xflags = LineFlags::USED | LineFlags::OUTPUT;

    // single line
    let lr = lr_base.clone();
    let l = get_line(&f, lr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_output");
    assert_eq!(info.flags, xflags);
    assert_eq!(s.get_level(offset).unwrap(), Level::Low);
    drop(l);

    // multiple lines
    let mut lr = lr_base.clone();
    lr.num_lines = 3;
    lr.offsets.set(1, 0);
    lr.offsets.set(2, 3);
    let l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_output");
    assert_eq!(info.flags, xflags);
    assert_eq!(s.get_level(offset).unwrap(), Level::Low);
    info = get_line_info(&f, 0).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_output");
    assert_eq!(info.flags, xflags);
    assert_eq!(s.get_level(0).unwrap(), Level::Low);
    info = get_line_info(&f, 3).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_output");
    assert_eq!(info.flags, xflags);
    assert_eq!(s.get_level(3).unwrap(), Level::Low);
    drop(l);
}

#[test]
fn with_output_values() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr_base = LineRequest {
        num_lines: 1,
        consumer: "with_output_values".into(),
        config: LineConfig {
            flags: LineFlags::OUTPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    lr_base.offsets.set(0, offset);
    lr_base.config.num_attrs = 1;
    let mut values = 1;
    let attr_mut = lr_base.config.attr_mut(0);
    attr_mut.attr.set_values(values);
    attr_mut.mask = 1;
    let xflags = LineFlags::USED | LineFlags::OUTPUT;

    // single line
    let lr = lr_base.clone();
    let l = get_line(&f, lr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_output_values"
    );
    assert_eq!(info.flags, xflags);
    assert_eq!(s.get_level(offset).unwrap(), Level::High);
    drop(l);

    // multiple lines
    let mut lr = lr_base.clone();
    lr.num_lines = 3;
    lr.offsets.set(1, 0);
    lr.offsets.set(2, 3);
    values |= 0x04;
    let attr_mut = lr.config.attr_mut(0);
    attr_mut.attr.set_values(values);
    attr_mut.mask |= 0x04;
    let l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_output_values"
    );
    assert_eq!(info.flags, xflags);
    assert_eq!(s.get_level(offset).unwrap(), Level::High);
    info = get_line_info(&f, 0).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_output_values"
    );
    assert_eq!(info.flags, xflags);
    assert_eq!(s.get_level(0).unwrap(), Level::Low);
    info = get_line_info(&f, 3).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_output_values"
    );
    assert_eq!(info.flags, xflags);
    assert_eq!(s.get_level(3).unwrap(), Level::High);
    drop(l);
}

#[test]
fn with_event_buffer_size() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr_base = LineRequest {
        num_lines: 1,
        consumer: "with_event_buffer_size".into(),
        config: LineConfig {
            flags: LineFlags::INPUT | LineFlags::EDGE_FALLING | LineFlags::EDGE_RISING,
            ..Default::default()
        },
        ..Default::default()
    };
    lr_base.offsets.set(0, offset);

    let mut lr = lr_base.clone();
    lr.event_buffer_size = 2;
    let l = get_line(&f, lr).unwrap();

    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();
    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();

    // events 1 and 2 were dropped when the kernel event buffer overflowed

    let mut buf = vec![0_u64; LineEdgeEvent::u64_size()];
    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    let mut event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.offset, offset);
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge);
    assert_eq!(event.seqno, 3);

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.offset, offset);
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge);
    assert_eq!(event.seqno, 4);

    assert!(!wait_event(&l, EVENT_WAIT_TIMEOUT).unwrap());

    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.offset, offset);
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge);
    assert_eq!(event.seqno, 5);

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.offset, offset);
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge);
    assert_eq!(event.seqno, 6);

    drop(l);
}

#[test]
fn with_rising_edge() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr = LineRequest {
        num_lines: 1,
        consumer: "with_rising_edge".into(),
        config: LineConfig {
            flags: LineFlags::INPUT | LineFlags::EDGE_RISING,
            ..Default::default()
        },
        ..Default::default()
    };
    lr.offsets.set(0, offset);

    let l = get_line(&f, lr).unwrap();

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
    assert_eq!(event.offset, offset);
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge);
    assert_eq!(event.seqno, 1);

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.offset, offset);
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge);
    assert_eq!(event.seqno, 2);

    assert!(!wait_event(&l, EVENT_WAIT_TIMEOUT).unwrap());

    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.offset, offset);
    assert_eq!(event.kind, LineEdgeEventKind::RisingEdge);
    assert_eq!(event.seqno, 3);

    assert!(!has_event(&l).unwrap());

    drop(l);
}

#[test]
fn with_falling_edge() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr = LineRequest {
        num_lines: 1,
        consumer: "with_falling_edge".into(),
        config: LineConfig {
            flags: LineFlags::INPUT | LineFlags::EDGE_FALLING,
            ..Default::default()
        },
        ..Default::default()
    };
    lr.offsets.set(0, offset);

    let l = get_line(&f, lr).unwrap();

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
    assert_eq!(event.offset, offset);
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge);
    assert_eq!(event.seqno, 1);

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.offset, offset);
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge);
    assert_eq!(event.seqno, 2);

    assert!(!wait_event(&l, EVENT_WAIT_TIMEOUT).unwrap());

    s.pullup(offset).unwrap();
    wait_propagation_delay();
    s.pulldown(offset).unwrap();
    wait_propagation_delay();

    assert!(has_event(&l).unwrap());
    assert_eq!(read_event(&l, &mut buf), Ok(LineEdgeEvent::u64_size()));
    event = LineEdgeEvent::from_slice(&buf).unwrap();
    assert_eq!(event.offset, offset);
    assert_eq!(event.kind, LineEdgeEventKind::FallingEdge);
    assert_eq!(event.seqno, 3);

    assert!(!has_event(&l).unwrap());

    drop(l);
}

#[test]
fn with_offset_out_of_range() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 4;
    let mut lr = LineRequest {
        num_lines: 1,
        consumer: "with_offset_out_of_range".into(),
        ..Default::default()
    };
    lr.offsets.set(0, offset);
    assert_eq!(
        get_line(&f, lr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn while_busy() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr_base = LineRequest {
        num_lines: 1,
        consumer: "while_busy".into(),
        ..Default::default()
    };
    lr_base.offsets.set(0, offset);
    let mut lr = lr_base.clone();
    let l = get_line(&f, lr);

    lr = lr_base.clone();
    assert_eq!(get_line(&f, lr).unwrap_err(), Error::Os(Errno(libc::EBUSY)));
    drop(l);
}

#[test]
fn with_extra_offsets() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let offsets = [2, 3];
    let mut lr = LineRequest {
        num_lines: 1,
        consumer: "with_extra_offsets".into(),
        ..Default::default()
    };
    lr.offsets.copy_from_slice(&offsets);
    let l = get_line(&f, lr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_extra_offsets"
    );
    assert_eq!(info.flags, LineFlags::USED | LineFlags::INPUT);
    info = get_line_info(&f, 3).unwrap();
    assert!(info.consumer.is_empty());
    assert_eq!(info.flags, LineFlags::INPUT);
    drop(l);
}

#[test]
fn with_repeated_offset() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr_base = LineRequest {
        num_lines: 2,
        consumer: "with_repeated_offset".into(),
        ..Default::default()
    };
    lr_base.offsets.set(0, offset);
    lr_base.offsets.set(1, offset);
    let lr = lr_base.clone();
    assert_eq!(get_line(&f, lr).unwrap_err(), Error::Os(Errno(libc::EBUSY)));
}

#[test]
fn with_multiple_bias_flags() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr_base = LineRequest {
        num_lines: 1,
        consumer: "with_multiple_bias_flags".into(),
        config: LineConfig {
            flags: LineFlags::OUTPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    lr_base.offsets.set(0, offset);

    let mut lr = lr_base.clone();
    lr.config.flags |= LineFlags::BIAS_PULL_UP;
    lr.config.flags |= LineFlags::BIAS_PULL_DOWN;
    assert_eq!(
        get_line(&f, lr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::BIAS_PULL_UP;
    lr.config.flags |= LineFlags::BIAS_DISABLED;
    assert_eq!(
        get_line(&f, lr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::BIAS_PULL_DOWN;
    lr.config.flags |= LineFlags::BIAS_DISABLED;
    assert_eq!(
        get_line(&f, lr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn with_multiple_drive_flags() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr_base = LineRequest {
        num_lines: 1,
        consumer: "with_multiple_drive_flags".into(),
        config: LineConfig {
            flags: LineFlags::OUTPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    lr_base.offsets.set(0, offset);

    let mut lr = lr_base.clone();
    lr.config.flags |= LineFlags::OPEN_DRAIN;
    lr.config.flags |= LineFlags::OPEN_SOURCE;
    assert_eq!(
        get_line(&f, lr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn with_bias_without_direction() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr_base = LineRequest {
        num_lines: 1,
        consumer: "with_bias_without_direction".into(),
        ..Default::default()
    };
    lr_base.offsets.set(0, offset);

    let mut lr = lr_base.clone();
    lr.config.flags |= LineFlags::BIAS_PULL_UP;
    assert_eq!(
        get_line(&f, lr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::BIAS_PULL_DOWN;
    assert_eq!(
        get_line(&f, lr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::BIAS_DISABLED;
    assert_eq!(
        get_line(&f, lr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn with_drive_without_output() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr_base = LineRequest {
        num_lines: 1,
        consumer: "with_drive_without_output".into(),
        ..Default::default()
    };
    lr_base.offsets.set(0, offset);

    // as-is
    let mut lr = lr_base.clone();
    lr.config.flags |= LineFlags::OPEN_DRAIN;
    assert_eq!(
        get_line(&f, lr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::OPEN_SOURCE;
    assert_eq!(
        get_line(&f, lr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    // input
    lr_base.config.flags |= LineFlags::INPUT;

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::OPEN_DRAIN;
    assert_eq!(
        get_line(&f, lr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::OPEN_SOURCE;
    assert_eq!(
        get_line(&f, lr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}
