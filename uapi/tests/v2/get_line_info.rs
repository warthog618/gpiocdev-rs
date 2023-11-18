// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use gpiocdev_uapi::v2::{get_line_info, LineFlags};

#[test]
fn check_info() {
    let s = crate::common::detailed_sim();

    for sc in s.chips() {
        let f = fs::File::open(sc.dev_path()).unwrap();
        // names, offsets and used
        for (offset, name) in &sc.config().names {
            let info = get_line_info(&f, *offset).unwrap();
            assert_eq!(info.name.as_os_str().to_string_lossy(), *name);
            assert_eq!(info.offset, *offset);
            if sc.config().hogs.contains_key(offset) {
                assert!(info.flags.contains(LineFlags::USED));
            } else {
                assert!(!info.flags.contains(LineFlags::USED));
            }
            assert!(!info.flags.contains(LineFlags::ACTIVE_LOW));
            assert!(!info.flags.contains(LineFlags::OPEN_DRAIN));
            assert!(!info.flags.contains(LineFlags::OPEN_SOURCE));
            assert!(!info.flags.contains(LineFlags::BIAS_PULL_UP));
            assert!(!info.flags.contains(LineFlags::BIAS_PULL_DOWN));
            assert!(!info.flags.contains(LineFlags::BIAS_DISABLED));
        }

        // consumer, direction and used
        for (offset, hog) in &sc.config().hogs {
            let info = get_line_info(&f, *offset).unwrap();
            assert_eq!(info.consumer.as_os_str().to_string_lossy(), hog.consumer);
            assert_eq!(info.offset, *offset);
            match &hog.direction {
                Direction::Input => assert!(!info.flags.contains(LineFlags::OUTPUT)),
                Direction::OutputLow => assert!(info.flags.contains(LineFlags::OUTPUT)),
                Direction::OutputHigh => {
                    assert!(info.flags.contains(LineFlags::OUTPUT))
                }
            }
            assert!(info.flags.contains(LineFlags::USED));
        }
    }
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let offset = 2;
    let mut lr_base = LineRequest {
        num_lines: 1,
        consumer: "check_info".into(),
        config: LineConfig {
            flags: LineFlags::OUTPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    lr_base.offsets.set(0, offset);
    let mut xflags = LineFlags::USED | LineFlags::OUTPUT;

    // active low
    let mut lr = lr_base.clone();
    lr.config.flags |= LineFlags::ACTIVE_LOW;
    let mut l = get_line(&f, lr).unwrap();
    let mut info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::ACTIVE_LOW);
    drop(l);

    // drive
    lr = lr_base.clone();
    lr.config.flags |= LineFlags::OPEN_DRAIN;
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::OPEN_DRAIN);
    drop(l);

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::OPEN_SOURCE;
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::OPEN_SOURCE);
    drop(l);

    // bias
    lr = lr_base.clone();
    lr.config.flags |= LineFlags::BIAS_PULL_UP;
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::BIAS_PULL_UP);
    drop(l);

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::BIAS_PULL_DOWN;
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::BIAS_PULL_DOWN);
    drop(l);

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::BIAS_DISABLED;
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::BIAS_DISABLED);
    drop(l);

    // edges
    lr_base.config.flags = LineFlags::INPUT;
    xflags = LineFlags::USED | LineFlags::INPUT;
    lr = lr_base.clone();
    lr.config.flags |= LineFlags::EDGE_RISING;
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::EDGE_RISING);
    drop(l);

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::EDGE_RISING;
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::EDGE_RISING);
    drop(l);

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::EDGE_FALLING;
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::EDGE_FALLING);
    drop(l);

    lr = lr_base.clone();
    lr.config.flags |= LineFlags::EDGE_RISING | LineFlags::EDGE_FALLING;
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(
        info.flags,
        xflags | LineFlags::EDGE_RISING | LineFlags::EDGE_FALLING
    );
    drop(l);

    // event clock
    xflags = LineFlags::USED | LineFlags::INPUT | LineFlags::EDGE_RISING;
    lr = lr_base.clone();
    lr.config.flags |= LineFlags::EDGE_RISING | LineFlags::EVENT_CLOCK_REALTIME;
    l = get_line(&f, lr).unwrap();
    info = get_line_info(&f, offset).unwrap();
    assert_eq!(info.consumer.as_os_str().to_string_lossy(), "check_info");
    assert_eq!(info.flags, xflags | LineFlags::EVENT_CLOCK_REALTIME);
    drop(l);

    // hte requires a HTE supporting kernel, which is uncommon.

    // debounce
    lr = lr_base.clone();
    lr.config.flags |= LineFlags::EDGE_RISING | LineFlags::EDGE_FALLING;
    lr.config.num_attrs = 1;
    let mut xattr = LineAttribute::default();
    xattr.set_debounce_period_us(123);
    let attr = lr.config.attr_mut(0);
    attr.mask = 1;
    attr.attr = xattr;
    l = get_line(&f, lr).unwrap();
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
fn with_offset_out_of_range() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    assert_eq!(
        get_line_info(&f, 4),
        Err(Error::Os(Errno(libc::EINVAL)))
    );
}
