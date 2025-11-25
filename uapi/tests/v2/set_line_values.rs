// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

use crate::common::wait_propagation_delay;

#[test]
fn on_output() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
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

    let l = get_line(&f, lr).expect("get_line should succeed");

    // uAPI defaults to pulling low
    assert_eq!(
        s.get_level(0).expect("get_level should succeed"),
        Level::Low
    );
    assert_eq!(
        s.get_level(1).expect("get_level should succeed"),
        Level::Low
    );
    assert_eq!(
        s.get_level(2).expect("get_level should succeed"),
        Level::Low
    );
    assert_eq!(
        s.get_level(3).expect("get_level should succeed"),
        Level::Low
    );

    let mut values = LineValues::default();
    values.copy_from_slice(&[true, false, false, true]);
    assert_eq!(set_line_values(&l, &values), Ok(()));
    wait_propagation_delay();
    assert_eq!(
        s.get_level(0).expect("get_level should succeed"),
        Level::High
    );
    assert_eq!(
        s.get_level(1).expect("get_level should succeed"),
        Level::Low
    );
    assert_eq!(
        s.get_level(2).expect("get_level should succeed"),
        Level::Low
    );
    assert_eq!(
        s.get_level(3).expect("get_level should succeed"),
        Level::High
    );

    values.copy_from_slice(&[false, true, false, true]);
    assert_eq!(set_line_values(&l, &values), Ok(()));
    wait_propagation_delay();
    assert_eq!(
        s.get_level(0).expect("get_level should succeed"),
        Level::Low
    );
    assert_eq!(
        s.get_level(1).expect("get_level should succeed"),
        Level::High
    );
    assert_eq!(
        s.get_level(2).expect("get_level should succeed"),
        Level::Low
    );
    assert_eq!(
        s.get_level(3).expect("get_level should succeed"),
        Level::High
    );
}

#[test]
fn with_extra_values() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
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

    let l = get_line(&f, lr).expect("get_line should succeed");

    // uAPI defaults to pulling low
    assert_eq!(
        s.get_level(0).expect("get_level should succeed"),
        Level::Low
    );
    assert_eq!(
        s.get_level(1).expect("get_level should succeed"),
        Level::Low
    );
    assert_eq!(
        s.get_level(2).expect("get_level should succeed"),
        Level::Low
    );
    assert_eq!(
        s.get_level(3).expect("get_level should succeed"),
        Level::Low
    );

    let mut values = LineValues::default();
    values.copy_from_slice(&[true, false, false, true]);
    assert_eq!(set_line_values(&l, &values), Ok(()));
    wait_propagation_delay();
    assert_eq!(
        s.get_level(0).expect("get_level should succeed"),
        Level::High
    );
    assert_eq!(
        s.get_level(1).expect("get_level should succeed"),
        Level::Low
    );
    assert_eq!(
        s.get_level(2).expect("get_level should succeed"),
        Level::Low
    );
    assert_eq!(
        s.get_level(3).expect("get_level should succeed"),
        Level::Low
    );
}

#[test]
fn on_input() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
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

    let l = get_line(&f, lr).expect("get_line should succeed");
    let values = LineValues::from_slice(&[true, false]);
    assert_eq!(
        set_line_values(&l, &values).expect_err("set_line_values should fail"),
        Error::Os(Errno(libc::EPERM))
    );
}
