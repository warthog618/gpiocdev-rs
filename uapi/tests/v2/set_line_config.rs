// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn check_info() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr = LineRequest {
        num_lines: 1,
        consumer: "check_info".into(),
        config: LineConfig {
            flags: LineFlags::OUTPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    lr.offsets.set(0, offset);

    let mut lc_base = LineConfig {
        flags: LineFlags::OUTPUT,
        ..Default::default()
    };

    // baseline
    let l = get_line(&f, lr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    let mut xflags = LineFlags::USED | LineFlags::OUTPUT;
    assert_eq!(info.flags, xflags);

    // active low
    let mut lc = lc_base.clone();
    lc.flags |= LineFlags::ACTIVE_LOW;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::ACTIVE_LOW);

    // bias
    lc = lc_base.clone();
    lc.flags |= LineFlags::BIAS_DISABLED;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::BIAS_DISABLED);

    lc = lc_base.clone();
    lc.flags |= LineFlags::BIAS_PULL_DOWN;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::BIAS_PULL_DOWN);

    lc = lc_base.clone();
    lc.flags |= LineFlags::BIAS_PULL_UP;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::BIAS_PULL_UP);

    // drive
    lc = lc_base.clone();
    lc.flags |= LineFlags::OPEN_DRAIN;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::OPEN_DRAIN);

    lc = lc_base.clone();
    lc.flags |= LineFlags::OPEN_SOURCE;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::OPEN_SOURCE);

    // direction
    lc = lc_base.clone();
    lc.flags = LineFlags::INPUT;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, LineFlags::USED | LineFlags::INPUT);

    // edges
    lc_base.flags = LineFlags::INPUT;
    xflags = LineFlags::USED | LineFlags::INPUT;
    lc = lc_base.clone();
    lc.flags |= LineFlags::EDGE_RISING;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::EDGE_RISING);

    lc = lc_base.clone();
    lc.flags |= LineFlags::EDGE_RISING;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::EDGE_RISING);

    lc = lc_base.clone();
    lc.flags |= LineFlags::EDGE_FALLING;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::EDGE_FALLING);

    lc = lc_base.clone();
    lc.flags |= LineFlags::EDGE_RISING | LineFlags::EDGE_FALLING;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(
        info.flags,
        xflags | LineFlags::EDGE_RISING | LineFlags::EDGE_FALLING
    );

    // event clock
    xflags = LineFlags::USED | LineFlags::INPUT | LineFlags::EDGE_RISING;
    lc = lc_base.clone();
    lc.flags |= LineFlags::EDGE_RISING | LineFlags::EVENT_CLOCK_REALTIME;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::EVENT_CLOCK_REALTIME);

    // hte requires a HTE supporting kernel, which is uncommon.

    // debounce
    lc = lc_base;
    lc.flags |= LineFlags::EDGE_RISING | LineFlags::EDGE_FALLING;
    lc.num_attrs = 1;
    let mut xattr = LineAttribute::default();
    xattr.set_debounce_period_us(123);
    let attr = lc.attr_mut(0);
    attr.mask = 1;
    attr.attr = xattr;
    assert_eq!(set_line_config(&l, lc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(
        info.flags,
        xflags | LineFlags::EDGE_RISING | LineFlags::EDGE_FALLING
    );
    assert_eq!(info.num_attrs, 1);
    assert_eq!(info.attr(0), &xattr);
    drop(l);
}

#[test]
fn with_multiple_bias_flags() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr = LineRequest {
        num_lines: 1,
        consumer: "with_multiple_bias_flags".into(),
        config: LineConfig {
            flags: LineFlags::OUTPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    lr.offsets.set(0, offset);

    let lc_base = LineConfig {
        flags: LineFlags::OUTPUT,
        ..Default::default()
    };

    // baseline
    let l = get_line(&f, lr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_multiple_bias_flags"
    );
    let xflags = LineFlags::USED | LineFlags::OUTPUT;
    assert_eq!(info.flags, xflags);

    let mut lc = lc_base.clone();
    lc.flags |= LineFlags::BIAS_PULL_UP;
    lc.flags |= LineFlags::BIAS_DISABLED;
    assert_eq!(
        set_line_config(&f, lc).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_multiple_bias_flags"
    );
    assert_eq!(info.flags, xflags);

    lc = lc_base.clone();
    lc.flags |= LineFlags::BIAS_PULL_UP;
    lc.flags |= LineFlags::BIAS_PULL_DOWN;
    assert_eq!(
        set_line_config(&f, lc).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_multiple_bias_flags"
    );
    assert_eq!(info.flags, xflags);

    lc = lc_base;
    lc.flags |= LineFlags::BIAS_DISABLED;
    lc.flags |= LineFlags::BIAS_PULL_DOWN;
    assert_eq!(
        set_line_config(&f, lc).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_multiple_bias_flags"
    );
    assert_eq!(info.flags, xflags);

    drop(l);
}

#[test]
fn with_multiple_drive_flags() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr = LineRequest {
        num_lines: 1,
        consumer: "with_multiple_drive_flags".into(),
        config: LineConfig {
            flags: LineFlags::OUTPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    lr.offsets.set(0, offset);

    let l = get_line(&f, lr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_multiple_drive_flags"
    );
    let xflags = LineFlags::USED | LineFlags::OUTPUT;
    assert_eq!(info.flags, xflags);

    let lc = LineConfig {
        flags: LineFlags::OUTPUT | LineFlags::OPEN_DRAIN | LineFlags::OPEN_SOURCE,
        ..Default::default()
    };
    assert_eq!(
        set_line_config(&f, lc).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_multiple_drive_flags"
    );
    assert_eq!(info.flags, xflags);

    drop(l);
}

#[test]
fn without_direction() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr = LineRequest {
        num_lines: 1,
        consumer: "without_direction".into(),
        config: LineConfig {
            flags: LineFlags::OUTPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    lr.offsets.set(0, offset);

    let l = get_line(&f, lr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "without_direction"
    );
    let xflags = LineFlags::USED | LineFlags::OUTPUT;
    assert_eq!(info.flags, xflags);

    let lc = LineConfig::default();
    assert_eq!(
        set_line_config(&f, lc).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "without_direction"
    );
    assert_eq!(info.flags, xflags);

    drop(l);
}
