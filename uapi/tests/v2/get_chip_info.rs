// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn check_info() {
    let s = gpiosim::builder()
        .with_bank(&Bank::new(8, "veintidós"))
        .with_bank(&Bank::new(12, "babel"))
        .with_bank(&Bank::new(6, "woggle"))
        .live()
        .unwrap();

    for sc in s.chips() {
        let f = fs::File::open(sc.dev_path()).unwrap();
        let info = get_chip_info(f.as_raw_fd()).unwrap();
        assert_eq!(info.num_lines, sc.config().num_lines);
        assert_eq!(info.label.as_os_str().to_string_lossy(), sc.config().label);
        assert_eq!(info.name.as_os_str().to_string_lossy(), sc.chip_name);
    }
}

#[test]
fn with_bad_fd() {
    let s = Simpleton::new(4);
    let f = fs::File::open(s.dev_path()).unwrap();
    let fd = f.as_raw_fd();
    drop(f);
    assert_eq!(get_chip_info(fd), Err(Error::Os(Errno(libc::EBADF))));
}
