// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

use crate::common::wait_propagation_delay;
use gpiocdev_uapi::v2::set_line_values;

#[test]
fn on_output() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let mut lr = LineRequest {
        num_lines: 4,
        consumer: "check_values".into(),
        config: LineConfig {
            flags: LineFlags::OUTPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    // doesn't have to be in order, but just keeping it simple...
    lr.offsets.copy_from_slice(&[0, 1, 2, 3]);

    let l = get_line(&f, lr).unwrap();

    // uAPI defaults to pulling low
    assert_eq!(s.get_level(0).unwrap(), Level::Low);
    assert_eq!(s.get_level(1).unwrap(), Level::Low);
    assert_eq!(s.get_level(2).unwrap(), Level::Low);
    assert_eq!(s.get_level(3).unwrap(), Level::Low);

    let mut values = LineValues::default();
    values.copy_from_slice(&[true, false, false, true]);
    assert_eq!(set_line_values(&l, &values), Ok(()));
    wait_propagation_delay();
    assert_eq!(s.get_level(0).unwrap(), Level::High);
    assert_eq!(s.get_level(1).unwrap(), Level::Low);
    assert_eq!(s.get_level(2).unwrap(), Level::Low);
    assert_eq!(s.get_level(3).unwrap(), Level::High);

    values.copy_from_slice(&[false, true, false, true]);
    assert_eq!(set_line_values(&l, &values), Ok(()));
    wait_propagation_delay();
    assert_eq!(s.get_level(0).unwrap(), Level::Low);
    assert_eq!(s.get_level(1).unwrap(), Level::High);
    assert_eq!(s.get_level(2).unwrap(), Level::Low);
    assert_eq!(s.get_level(3).unwrap(), Level::High);
}

#[test]
fn with_extra_values() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let mut lr = LineRequest {
        num_lines: 2,
        consumer: "with_extra_values".into(),
        config: LineConfig {
            flags: LineFlags::OUTPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    // doesn't have to be in order, but just keeping it simple...
    lr.offsets.copy_from_slice(&[0, 1]);

    let l = get_line(&f, lr).unwrap();

    // uAPI defaults to pulling low
    assert_eq!(s.get_level(0).unwrap(), Level::Low);
    assert_eq!(s.get_level(1).unwrap(), Level::Low);
    assert_eq!(s.get_level(2).unwrap(), Level::Low);
    assert_eq!(s.get_level(3).unwrap(), Level::Low);

    let mut values = LineValues::default();
    values.copy_from_slice(&[true, false, false, true]);
    assert_eq!(set_line_values(&l, &values), Ok(()));
    wait_propagation_delay();
    assert_eq!(s.get_level(0).unwrap(), Level::High);
    assert_eq!(s.get_level(1).unwrap(), Level::Low);
    assert_eq!(s.get_level(2).unwrap(), Level::Low);
    assert_eq!(s.get_level(3).unwrap(), Level::Low);
}

#[test]
fn on_input() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let mut lr = LineRequest {
        num_lines: 2,
        consumer: "on_input".into(),
        config: LineConfig {
            flags: LineFlags::INPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    // doesn't have to be in order, but just keeping it simple...
    lr.offsets.copy_from_slice(&[0, 1]);

    let l = get_line(&f, lr).unwrap();
    let values = LineValues::from_slice(&[true, false]);
    assert_eq!(
        set_line_values(&l, &values).unwrap_err(),
        Error::Os(Errno(libc::EPERM))
    );
}
