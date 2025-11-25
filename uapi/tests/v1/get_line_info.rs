// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn check_info() {
    let s = crate::common::detailed_sim();

    for sc in s.chips() {
        let f = fs::File::open(sc.dev_path()).expect("gpiosim chip should exist");
        // names, offsets and used
        for offset in 0..sc.config().num_lines {
            let info = get_line_info(&f, offset).expect("get_line_info should succeed");
            assert_eq!(info.offset, offset);
            if let Some(name) = sc.config().names.get(&offset) {
                assert_eq!(info.name.as_os_str().to_string_lossy(), *name);
            } else {
                assert!(info.name.is_empty());
            }
            if let Some(hog) = sc.config().hogs.get(&offset) {
                let mut xflags = LineInfoFlags::USED;
                if Direction::Input != hog.direction {
                    xflags |= LineInfoFlags::OUTPUT;
                }
                assert_eq!(info.flags, xflags);
            } else {
                assert!(info.flags.is_empty());
            }
        }

        // consumer, direction and used
        for (offset, hog) in &sc.config().hogs {
            let info = get_line_info(&f, *offset).expect("get_line_info should succeed");
            assert_eq!(info.consumer.as_os_str().to_string_lossy(), hog.consumer);
            assert_eq!(info.offset, *offset);
            match &hog.direction {
                Direction::Input => assert!(!info.flags.contains(LineInfoFlags::OUTPUT)),
                Direction::OutputLow => assert!(info.flags.contains(LineInfoFlags::OUTPUT)),
                Direction::OutputHigh => {
                    assert!(info.flags.contains(LineInfoFlags::OUTPUT))
                }
            }
            assert!(info.flags.contains(LineInfoFlags::USED));
        }
    }
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let offset = 2;
    let mut hr_base = HandleRequest {
        num_lines: 1,
        consumer: "check_info".into(),
        flags: HandleRequestFlags::OUTPUT,
        ..Default::default()
    };
    hr_base.offsets.set(0, offset);

    // active low
    let mut hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::ACTIVE_LOW;
    let mut h = get_line_handle(&f, hr).expect("get_line_handle should succeed");
    let mut info = get_line_info(&f, offset).expect("get_line_info should succeed");
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    let mut xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT | LineInfoFlags::ACTIVE_LOW;
    assert_eq!(info.flags, xflags);
    drop(h);

    // drive
    hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::OPEN_DRAIN;
    h = get_line_handle(&f, hr).expect("get_line_handle should succeed");
    info = get_line_info(&f, offset).expect("get_line_info should succeed");
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT | LineInfoFlags::OPEN_DRAIN;
    assert_eq!(info.flags, xflags);
    drop(h);

    hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::OPEN_SOURCE;
    h = get_line_handle(&f, hr).expect("get_line_handle should succeed");
    info = get_line_info(&f, offset).expect("get_line_info should succeed");
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT | LineInfoFlags::OPEN_SOURCE;
    assert_eq!(info.flags, xflags);
    drop(h);

    // bias
    hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::BIAS_PULL_UP;
    h = get_line_handle(&f, hr).expect("get_line_handle should succeed");
    info = get_line_info(&f, offset).expect("get_line_info should succeed");
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT | LineInfoFlags::BIAS_PULL_UP;
    assert_eq!(info.flags, xflags);
    drop(h);

    hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::BIAS_PULL_DOWN;
    h = get_line_handle(&f, hr).expect("get_line_handle should succeed");
    info = get_line_info(&f, offset).expect("get_line_info should succeed");
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT | LineInfoFlags::BIAS_PULL_DOWN;
    assert_eq!(info.flags, xflags);
    drop(h);

    hr = hr_base.clone();
    hr.flags |= HandleRequestFlags::BIAS_DISABLED;
    h = get_line_handle(&f, hr).expect("get_line_handle should succeed");
    info = get_line_info(&f, offset).expect("get_line_info should succeed");
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    xflags = LineInfoFlags::USED | LineInfoFlags::OUTPUT | LineInfoFlags::BIAS_DISABLED;
    assert_eq!(info.flags, xflags);
    drop(h);

    // v1 does not return edge detection in line info
}

#[test]
fn with_offset_out_of_range() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    assert_eq!(get_line_info(&f, 4), Err(Error::Os(Errno(libc::EINVAL))));
}
