// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::common::wait_propagation_delay;

#[test]
fn on_output() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let mut hr = HandleRequest {
        num_lines: 4,
        flags: HandleRequestFlags::OUTPUT,
        consumer: "on_output".into(),
        ..Default::default()
    };
    // doesn't have to be in order, but just keeping it simple...
    hr.offsets.copy_from_slice(&[0, 1, 2, 3]);

    let l = get_line_handle(&f, hr).unwrap();

    // uAPI defaults to pulling low
    assert_eq!(s.get_level(0).unwrap(), Level::Low);
    assert_eq!(s.get_level(1).unwrap(), Level::Low);
    assert_eq!(s.get_level(2).unwrap(), Level::Low);
    assert_eq!(s.get_level(3).unwrap(), Level::Low);

    let mut values = LineValues::default();
    values.copy_from_slice(&[1, 0, 0, 1]);
    assert_eq!(set_line_values(&l, &values), Ok(()));
    wait_propagation_delay();
    assert_eq!(s.get_level(0).unwrap(), Level::High);
    assert_eq!(s.get_level(1).unwrap(), Level::Low);
    assert_eq!(s.get_level(2).unwrap(), Level::Low);
    assert_eq!(s.get_level(3).unwrap(), Level::High);

    values.copy_from_slice(&[0, 1, 0, 1]);
    assert_eq!(set_line_values(&l, &values), Ok(()));
    wait_propagation_delay();
    assert_eq!(s.get_level(0).unwrap(), Level::Low);
    assert_eq!(s.get_level(1).unwrap(), Level::High);
    assert_eq!(s.get_level(2).unwrap(), Level::Low);
    assert_eq!(s.get_level(3).unwrap(), Level::High);
}

#[test]
fn on_input() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let mut hr = HandleRequest {
        num_lines: 2,
        flags: HandleRequestFlags::INPUT,
        consumer: "on_input".into(),
        ..Default::default()
    };
    // doesn't have to be in order, but just keeping it simple...
    hr.offsets.copy_from_slice(&[0, 1]);

    let l = get_line_handle(&f, hr).unwrap();
    let values = LineValues::from_slice(&[1, 0]);
    assert_eq!(
        set_line_values(&l, &values).unwrap_err(),
        Error::Os(Errno(libc::EPERM))
    );
}
