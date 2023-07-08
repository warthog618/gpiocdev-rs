// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::common::wait_propagation_delay;
use gpiocdev_uapi::v2::{get_line_values, set_line_values};

#[test]
fn on_input() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let fd = f.as_raw_fd();
    let mut lr = LineRequest {
        num_lines: 4,
        consumer: "on_input".into(),
        config: LineConfig {
            flags: LineFlags::INPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    // doesn't have to be in order, but just keeping it simple...
    lr.offsets.copy_from_slice(&[0, 1, 2, 3]);

    let l = get_line(fd, lr).unwrap();
    let lfd = l.as_raw_fd();

    // sim defaults to pulling low
    let mut values = LineValues::from_slice(&[true, true, true, true]);
    get_line_values(lfd, &mut values).unwrap();
    assert_eq!(values.get(0), Some(false));
    assert_eq!(values.get(1), Some(false));
    assert_eq!(values.get(2), Some(false));
    assert_eq!(values.get(3), Some(false));

    s.set_pull(0, Level::High).unwrap();
    s.set_pull(3, Level::High).unwrap();
    wait_propagation_delay();
    get_line_values(lfd, &mut values).unwrap();
    assert_eq!(values.get(0), Some(true));
    assert_eq!(values.get(1), Some(false));
    assert_eq!(values.get(2), Some(false));
    assert_eq!(values.get(3), Some(true));

    s.set_pull(0, Level::Low).unwrap();
    s.set_pull(1, Level::High).unwrap();
    wait_propagation_delay();
    get_line_values(lfd, &mut values).unwrap();
    assert_eq!(values.get(0), Some(false));
    assert_eq!(values.get(1), Some(true));
    assert_eq!(values.get(2), Some(false));
    assert_eq!(values.get(3), Some(true));

    // subsets
    // Note that the purpose of subsets is to reduce the number of lines physically
    // retrieved, as that may be faster for large sets where only changes to
    // a small number of lines are of interest at a given time.
    // The ioctl does NOT guarantee to leave the unmasked bits untouched - in fact
    // they are reset to 0 by the kernel - which is tested here too.
    // So the passed LineValues cannot be used as a store for the latest known values
    // of all lines in the request.
    values.unset_mask(1);
    values.unset_mask(2);
    s.set_pull(0, Level::High).unwrap();
    s.set_pull(1, Level::Low).unwrap();
    s.set_pull(2, Level::High).unwrap();
    s.set_pull(3, Level::Low).unwrap();
    wait_propagation_delay();
    assert_eq!(get_line_values(lfd, &mut values), Ok(()));
    assert_eq!(values.get(0), Some(true));
    assert!(!values.bits.get(1)); // reset to 0 by the kernel despite the mask being clear.
    assert!(!values.bits.get(2)); // also reset to 0, not reflecting the value of the line.
    assert_eq!(values.get(3), Some(false));
}

#[test]
fn on_output() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let fd = f.as_raw_fd();
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

    let l = get_line(fd, lr).unwrap();
    let lfd = l.as_raw_fd();
    let mut values = LineValues::from_slice(&[true, true, true, true]);

    // uAPI defaults to pulling low
    assert_eq!(s.get_level(0).unwrap(), Level::Low);
    assert_eq!(s.get_level(1).unwrap(), Level::Low);
    assert_eq!(s.get_level(2).unwrap(), Level::Low);
    assert_eq!(s.get_level(3).unwrap(), Level::Low);
    get_line_values(lfd, &mut values).unwrap();
    assert_eq!(values.get(0), Some(false));
    assert_eq!(values.get(1), Some(false));
    assert_eq!(values.get(2), Some(false));
    assert_eq!(values.get(3), Some(false));

    values.copy_from_slice(&[true, false, false, true]);
    assert_eq!(set_line_values(lfd, &values), Ok(()));
    wait_propagation_delay();
    assert_eq!(s.get_level(0).unwrap(), Level::High);
    assert_eq!(s.get_level(1).unwrap(), Level::Low);
    assert_eq!(s.get_level(2).unwrap(), Level::Low);
    assert_eq!(s.get_level(3).unwrap(), Level::High);
    get_line_values(lfd, &mut values).unwrap();
    assert_eq!(values.get(0), Some(true));
    assert_eq!(values.get(1), Some(false));
    assert_eq!(values.get(2), Some(false));
    assert_eq!(values.get(3), Some(true));

    values.copy_from_slice(&[false, true, false, true]);
    assert_eq!(set_line_values(lfd, &values), Ok(()));
    wait_propagation_delay();
    assert_eq!(s.get_level(0).unwrap(), Level::Low);
    assert_eq!(s.get_level(1).unwrap(), Level::High);
    assert_eq!(s.get_level(2).unwrap(), Level::Low);
    assert_eq!(s.get_level(3).unwrap(), Level::High);
    get_line_values(lfd, &mut values).unwrap();
    assert_eq!(values.get(0), Some(false));
    assert_eq!(values.get(1), Some(true));
    assert_eq!(values.get(2), Some(false));
    assert_eq!(values.get(3), Some(true));
}

#[test]
fn with_no_values() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let fd = f.as_raw_fd();
    let mut lr = LineRequest {
        num_lines: 4,
        consumer: "with_no_values".into(),
        config: LineConfig {
            flags: LineFlags::INPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    // doesn't have to be in order, but just keeping it simple...
    lr.offsets.copy_from_slice(&[0, 1, 2, 3]);

    let l = get_line(fd, lr).unwrap();
    let lfd = l.as_raw_fd();

    let mut values = LineValues::default();
    assert_eq!(
        get_line_values(lfd, &mut values).unwrap_err(),
        Error::Os(Errno(libc::EINVAL))
    );
}

#[test]
fn with_extra_bits() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let fd = f.as_raw_fd();
    let mut lr = LineRequest {
        num_lines: 4,
        consumer: "with_extra_bits".into(),
        config: LineConfig {
            flags: LineFlags::INPUT,
            ..Default::default()
        },
        ..Default::default()
    };
    // doesn't have to be in order, but just keeping it simple...
    lr.offsets.copy_from_slice(&[0, 1, 2, 3]);

    let l = get_line(fd, lr).unwrap();
    let lfd = l.as_raw_fd();

    let mut values = LineValues::from_slice(&[true, true, true, true, true, true]);
    get_line_values(lfd, &mut values).unwrap();
    assert_eq!(values.get(0), Some(false));
    assert_eq!(values.get(1), Some(false));
    assert_eq!(values.get(2), Some(false));
    assert_eq!(values.get(3), Some(false));
    assert_eq!(values.get(4), Some(false)); // ignored in the request but cleared in the return
    assert_eq!(values.get(5), Some(false)); // ignored in the request but cleared in the return
}

#[test]
fn with_bad_fd() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let fd = f.as_raw_fd();
    drop(f);
    let mut values = LineValues::from_slice(&[true, true, true]);
    assert_eq!(
        get_line_values(fd, &mut values),
        Err(Error::Os(Errno(libc::EBADF)))
    );
}
