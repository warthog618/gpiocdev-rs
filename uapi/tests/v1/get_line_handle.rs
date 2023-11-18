// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn as_is() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut hr_base = HandleRequest {
        num_lines: 1,
        consumer: "as_is".into(),
        ..Default::default()
    };
    hr_base.offsets.set(0, offset);

    // input

    // single line
    let mut hr = hr_base.clone();
    let mut l = get_line_handle(&f, hr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, LineInfoFlags::USED);
    drop(l);

    // multiple lines
    let offsets = [offset, 0, 3];
    hr = hr_base.clone();
    hr.num_lines = 3;
    hr.offsets.copy_from_slice(&offsets);
    l = get_line_handle(&f, hr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, LineInfoFlags::USED);
    info = get_line_info(&f, 0).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, LineInfoFlags::USED);
    info = get_line_info(&f, 3).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, LineInfoFlags::USED);
    drop(l);

    // force lines to output
    hr = hr_base.clone();
    hr.num_lines = 3;
    hr.offsets.copy_from_slice(&offsets);
    hr.flags = HandleRequestFlags::OUTPUT;
    l = get_line_handle(&f, hr).unwrap();
    drop(l);

    // output
    let xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT;

    // single line
    let mut hr = hr_base.clone();
    l = get_line_handle(&f, hr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_is");
    assert_eq!(info.flags, xflags);
    drop(l);

    // multiple lines
    hr = hr_base.clone();
    hr.num_lines = 3;
    hr.offsets.copy_from_slice(&offsets);
    l = get_line_handle(&f, hr).unwrap();
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
}

#[test]
fn as_input() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut hr_base = HandleRequest {
        num_lines: 1,
        consumer: "as_input".into(),
        flags: HandleRequestFlags::INPUT,
        ..Default::default()
    };
    hr_base.offsets.set(0, offset);

    // single line
    let hr = hr_base.clone();
    let mut l: fs::File = get_line_handle(&f, hr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_input");
    assert_eq!(info.flags, LineInfoFlags::USED);
    drop(l);

    // multiple lines
    let offsets = [offset, 0, 3];
    let mut hr = hr_base.clone();
    hr.num_lines = 3;
    hr.offsets.copy_from_slice(&offsets);
    l = get_line_handle(&f, hr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_input");
    assert_eq!(info.flags, LineInfoFlags::USED);
    info = get_line_info(&f, 0).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_input");
    assert_eq!(info.flags, LineInfoFlags::USED);
    info = get_line_info(&f, 3).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_input");
    assert_eq!(info.flags, LineInfoFlags::USED);
    drop(l);
}

#[test]
fn as_output() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut hr_base = HandleRequest {
        num_lines: 1,
        consumer: "as_output".into(),
        flags: HandleRequestFlags::OUTPUT,
        ..Default::default()
    };
    hr_base.offsets.set(0, offset);
    let xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT;

    // single line
    let hr = hr_base.clone();
    let mut l = get_line_handle(&f, hr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "as_output");
    assert_eq!(info.flags, xflags);
    assert_eq!(s.get_level(offset).unwrap(), Level::Low);
    drop(l);

    // multiple lines
    let offsets = [offset, 0, 3];
    let mut hr = hr_base.clone();
    hr.num_lines = 3;
    hr.offsets.copy_from_slice(&offsets);
    l = get_line_handle(&f, hr).unwrap();
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
    let mut hr_base = HandleRequest {
        num_lines: 1,
        consumer: "with_output_values".into(),
        flags: HandleRequestFlags::OUTPUT,
        ..Default::default()
    };
    hr_base.offsets.set(0, offset);
    hr_base.values.set(0, 1);
    let xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT;

    // single line
    let hr = hr_base.clone();
    let mut l = get_line_handle(&f, hr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_output_values"
    );
    assert_eq!(info.flags, xflags);
    assert_eq!(s.get_level(offset).unwrap(), Level::High);
    drop(l);

    // multiple lines
    let mut hr = hr_base.clone();
    hr.num_lines = 3;
    hr.offsets.set(1, 0);
    hr.offsets.set(2, 3);
    hr.values.set(2, 1);
    l = get_line_handle(&f, hr).unwrap();
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
fn with_offset_out_of_range() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 4;
    let mut hr = HandleRequest {
        num_lines: 1,
        consumer: "with_offset_out_of_range".into(),
        ..Default::default()
    };
    hr.offsets.set(0, offset);
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn while_busy() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut hr = HandleRequest {
        num_lines: 1,
        consumer: "while_busy".into(),
        ..Default::default()
    };
    hr.offsets.set(0, offset);
    let l = get_line_handle(&f, hr.clone());

    assert_eq!(
        get_line_handle(&f, hr.clone()).unwrap_err(),
        Error::Os(Errno(libc::EBUSY))
    );
    drop(l);
}

#[test]
fn with_extra_offsets() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let offsets = [2, 3];
    let mut hr = HandleRequest {
        num_lines: 1,
        consumer: "with_extra_offsets".into(),
        ..Default::default()
    };
    hr.offsets.copy_from_slice(&offsets);
    let l = get_line_handle(&f, hr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(
        info.consumer.as_os_str().to_string_lossy(),
        "with_extra_offsets"
    );
    assert_eq!(info.flags, LineInfoFlags::USED);
    info = get_line_info(&f, 3).unwrap();
    assert!(info.consumer.is_empty());
    assert!(info.flags.is_empty());
    drop(l);
}

#[test]
fn with_repeated_offset() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut hr = HandleRequest {
        num_lines: 2,
        consumer: "with_repeated_offset".into(),
        ..Default::default()
    };
    hr.offsets.set(0, offset);
    hr.offsets.set(1, offset);
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EBUSY))
    );
}

#[test]
fn with_multiple_bias_flags() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut hr_base = HandleRequest {
        num_lines: 1,
        consumer: "with_multiple_bias_flags".into(),
        flags: HandleRequestFlags::OUTPUT,
        ..Default::default()
    };
    hr_base.offsets.set(0, offset);

    let mut hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::BIAS_PULL_UP;
    hr.flags |= HandleRequestFlags::BIAS_PULL_DOWN;
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::BIAS_PULL_UP;
    hr.flags |= HandleRequestFlags::BIAS_DISABLED;
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::BIAS_PULL_DOWN;
    hr.flags |= HandleRequestFlags::BIAS_DISABLED;
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
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
    hr.flags |= HandleRequestFlags::OPEN_DRAIN;
    hr.flags |= HandleRequestFlags::OPEN_SOURCE;
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn with_bias_without_direction() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut hr_base = HandleRequest {
        num_lines: 1,
        consumer: "with_bias_without_direction".into(),
        ..Default::default()
    };
    hr_base.offsets.set(0, offset);

    let mut hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::BIAS_PULL_UP;
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::BIAS_PULL_DOWN;
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::BIAS_DISABLED;
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn with_drive_without_output() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut hr_base = HandleRequest {
        num_lines: 1,
        consumer: "with_drive_without_output".into(),
        ..Default::default()
    };
    hr_base.offsets.set(0, offset);

    // as-is
    let mut hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::OPEN_DRAIN;
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::OPEN_SOURCE;
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    // input
    hr_base.flags |= HandleRequestFlags::INPUT;

    hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::OPEN_DRAIN;
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );

    hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::OPEN_SOURCE;
    assert_eq!(
        get_line_handle(&f, hr).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}
