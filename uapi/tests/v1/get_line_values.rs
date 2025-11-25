// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::common::wait_propagation_delay;

#[test]
fn on_input() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let mut hr = HandleRequest {
        num_lines: 4,
        consumer: "on_input".into(),
        flags: HandleRequestFlags::INPUT,
        ..Default::default()
    };
    // doesn't have to be in order, but just keeping it simple...
    hr.offsets.copy_from_slice(&[0, 1, 2, 3]);

    let l = get_line_handle(&f, hr).expect("get_line_handle should succeed");

    // sim defaults to pulling low
    let mut values = LineValues::default();
    get_line_values(&l, &mut values).expect("get_line_values should succeed");
    assert_eq!(values.get(0), 0);
    assert_eq!(values.get(1), 0);
    assert_eq!(values.get(2), 0);
    assert_eq!(values.get(3), 0);

    s.set_pull(0, Level::High).expect("set_pull should succeed");
    s.set_pull(3, Level::High).expect("set_pull should succeed");
    wait_propagation_delay();
    get_line_values(&l, &mut values).expect("get_line_values should succeed");
    assert_eq!(values.get(0), 1);
    assert_eq!(values.get(1), 0);
    assert_eq!(values.get(2), 0);
    assert_eq!(values.get(3), 1);

    s.set_pull(0, Level::Low).expect("set_pull should succeed");
    s.set_pull(1, Level::High).expect("set_pull should succeed");
    wait_propagation_delay();
    get_line_values(&l, &mut values).expect("get_line_values should succeed");
    assert_eq!(values.get(0), 0);
    assert_eq!(values.get(1), 1);
    assert_eq!(values.get(2), 0);
    assert_eq!(values.get(3), 1);
}

#[test]
fn on_output() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
    let mut hr = HandleRequest {
        num_lines: 4,
        consumer: "on_output".into(),
        flags: HandleRequestFlags::OUTPUT,
        ..Default::default()
    };
    // doesn't have to be in order, but just keeping it simple...
    hr.offsets.copy_from_slice(&[0, 1, 2, 3]);

    let l = get_line_handle(&f, hr).expect("get_line_handle should succeed");

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
    get_line_values(&l, &mut values).expect("get_line_values should succeed");
    assert_eq!(values.get(0), 0);
    assert_eq!(values.get(1), 0);
    assert_eq!(values.get(2), 0);
    assert_eq!(values.get(3), 0);

    values.copy_from_slice(&[1, 0, 0, 1]);
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
    get_line_values(&l, &mut values).expect("get_line_values should succeed");
    assert_eq!(values.get(0), 1);
    assert_eq!(values.get(1), 0);
    assert_eq!(values.get(2), 0);
    assert_eq!(values.get(3), 1);

    values.copy_from_slice(&[0, 1, 0, 1]);
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
    get_line_values(&l, &mut values).expect("get_line_values should succeed");
    assert_eq!(values.get(0), 0);
    assert_eq!(values.get(1), 1);
    assert_eq!(values.get(2), 0);
    assert_eq!(values.get(3), 1);
}
