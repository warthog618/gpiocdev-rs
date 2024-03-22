// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn check_info() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut hr = HandleRequest {
        num_lines: 1,
        consumer: "check_info".into(),
        flags: HandleRequestFlags::OUTPUT,
        ..Default::default()
    };
    hr.offsets.set(0, offset);

    let hc_base = HandleConfig {
        flags: HandleRequestFlags::OUTPUT,
        ..Default::default()
    };

    // baseline
    let l = get_line_handle(&f, hr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    let xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT;
    assert_eq!(info.flags, xflags);

    // active low
    let mut hc = hc_base.clone();
    hc.flags |= HandleRequestFlags::ACTIVE_LOW;
    assert_eq!(set_line_config(&l, hc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineInfoFlags::ACTIVE_LOW);

    // bias
    let mut hc = hc_base.clone();
    hc.flags |= HandleRequestFlags::BIAS_DISABLED;
    assert_eq!(set_line_config(&l, hc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineInfoFlags::BIAS_DISABLED);

    hc = hc_base.clone();
    hc.flags |= HandleRequestFlags::BIAS_PULL_DOWN;
    assert_eq!(set_line_config(&l, hc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineInfoFlags::BIAS_PULL_DOWN);

    hc = hc_base.clone();
    hc.flags |= HandleRequestFlags::BIAS_PULL_UP;
    assert_eq!(set_line_config(&l, hc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineInfoFlags::BIAS_PULL_UP);

    // drive
    hc = hc_base.clone();
    hc.flags |= HandleRequestFlags::OPEN_DRAIN;
    assert_eq!(set_line_config(&l, hc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineInfoFlags::OPEN_DRAIN);

    hc = hc_base.clone();
    hc.flags |= HandleRequestFlags::OPEN_SOURCE;
    assert_eq!(set_line_config(&l, hc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineInfoFlags::OPEN_SOURCE);

    // direction
    hc = hc_base;
    hc.flags = HandleRequestFlags::INPUT;
    assert_eq!(set_line_config(&l, hc), Ok(()));
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, LineInfoFlags::USED);

    drop(l);
    // v1 can't enable edge detection in set_line_config
}

#[test]
fn with_multiple_bias_flags() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut hr = HandleRequest {
        num_lines: 1,
        consumer: "with_multiple_bias_flags".into(),
        flags: HandleRequestFlags::OUTPUT,
        ..Default::default()
    };
    hr.offsets.set(0, offset);

    let hc_base = HandleConfig {
        flags: HandleRequestFlags::OUTPUT,
        ..Default::default()
    };

    // baseline
    let l = get_line_handle(&f, hr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_multiple_bias_flags"
    );
    let xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT;
    assert_eq!(info.flags, xflags);

    let mut hc = hc_base.clone();
    hc.flags |= HandleRequestFlags::BIAS_PULL_UP;
    hc.flags |= HandleRequestFlags::BIAS_DISABLED;
    assert_eq!(
        set_line_config(&f, hc).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_multiple_bias_flags"
    );
    assert_eq!(info.flags, xflags);

    hc = hc_base.clone();
    hc.flags |= HandleRequestFlags::BIAS_PULL_UP;
    hc.flags |= HandleRequestFlags::BIAS_PULL_DOWN;
    assert_eq!(
        set_line_config(&f, hc).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_multiple_bias_flags"
    );
    assert_eq!(info.flags, xflags);

    hc = hc_base;
    hc.flags |= HandleRequestFlags::BIAS_DISABLED;
    hc.flags |= HandleRequestFlags::BIAS_PULL_DOWN;
    assert_eq!(
        set_line_config(&f, hc).unwrap_err(),
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
    let mut hr = HandleRequest {
        num_lines: 1,
        consumer: "with_multiple_drive_flags".into(),
        flags: HandleRequestFlags::OUTPUT,
        ..Default::default()
    };
    hr.offsets.set(0, offset);

    let l = get_line_handle(&f, hr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_multiple_drive_flags"
    );
    let xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT;
    assert_eq!(info.flags, xflags);

    let hc = HandleConfig {
        flags: HandleRequestFlags::OUTPUT
            | HandleRequestFlags::OPEN_DRAIN
            | HandleRequestFlags::OPEN_SOURCE,
        ..Default::default()
    };
    assert_eq!(
        set_line_config(&f, hc).unwrap_err(),
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
    let mut hr = HandleRequest {
        num_lines: 1,
        consumer: "without_direction".into(),
        flags: HandleRequestFlags::OUTPUT,
        ..Default::default()
    };
    hr.offsets.set(0, offset);

    let l = get_line_handle(&f, hr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "without_direction"
    );
    let xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT;
    assert_eq!(info.flags, xflags);

    let hc = HandleConfig::default();
    assert_eq!(
        set_line_config(&f, hc).unwrap_err(),
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
