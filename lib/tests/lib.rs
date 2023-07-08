// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// ALL IT as can't construct a Chip without opening a GPIO file.
//
// Assumptions:
//  - kernel supports uAPI versions corresponding to selected build features

use gpiosim::Bank;
use human_sort::compare;
use std::cmp::Ordering;

#[test]
fn find_named_line() {
    let sim = gpiosim::builder()
        .with_bank(
            Bank::new(8, "find_line 1")
                .name(3, "fl banana")
                .name(6, "fl apple"),
        )
        .with_bank(
            Bank::new(42, "find_line 2")
                .name(3, "fl piñata")
                .name(4, "fl piggly")
                .name(5, "fl apple"),
        )
        .live()
        .unwrap();

    let l = gpiocdev::find_named_line("fl banana").unwrap();
    assert_eq!(l.chip, *sim.chips()[0].dev_path());
    assert_eq!(l.info.offset, 3);

    let l = gpiocdev::find_named_line("fl piggly").unwrap();
    assert_eq!(&l.chip, sim.chips()[1].dev_path());
    assert_eq!(l.info.offset, 4);

    let l = gpiocdev::find_named_line("fl apple").unwrap();
    // depending on how other tests are running, the order of the sim chips is not 100% predictable.
    if compare(
        &sim.chips()[0].dev_path().to_string_lossy(),
        &sim.chips()[1].dev_path().to_string_lossy(),
    ) == Ordering::Less
    {
        assert_eq!(&l.chip, sim.chips()[0].dev_path());
        assert_eq!(l.info.offset, 6);
    } else {
        assert_eq!(&l.chip, sim.chips()[1].dev_path());
        assert_eq!(l.info.offset, 5);
    }

    assert!(gpiocdev::find_named_line("fl nada").is_none())
}

#[test]
fn find_named_lines() {
    let sim = gpiosim::builder()
        .with_bank(
            Bank::new(8, "find_lines 1")
                .name(3, "fls banana")
                .name(6, "fls apple"),
        )
        .with_bank(
            Bank::new(42, "find_lines 2")
                .name(3, "fls piñata")
                .name(4, "fls piggly")
                .name(5, "fls apple"),
        )
        .live()
        .unwrap();

    let found = gpiocdev::find_named_lines(&["fls banana"], true).unwrap();
    assert_eq!(found.len(), 1);
    let l = found.get(&"fls banana").unwrap();
    assert_eq!(&l.chip, sim.chips()[0].dev_path());
    assert_eq!(l.info.offset, 3);

    let found = gpiocdev::find_named_lines(&["fls piggly"], true).unwrap();
    assert_eq!(found.len(), 1);
    let l = found.get(&"fls piggly").unwrap();
    assert_eq!(&l.chip, sim.chips()[1].dev_path());
    assert_eq!(l.info.offset, 4);

    let found = gpiocdev::find_named_lines(&["fls apple"], false).unwrap();
    assert_eq!(found.len(), 1);
    let l = found.get(&"fls apple").unwrap();
    if compare(
        &sim.chips()[0].dev_path().to_string_lossy(),
        &sim.chips()[1].dev_path().to_string_lossy(),
    ) == Ordering::Less
    {
        assert_eq!(&l.chip, sim.chips()[0].dev_path());
        assert_eq!(l.info.offset, 6);
    } else {
        assert_eq!(&l.chip, sim.chips()[1].dev_path());
        assert_eq!(l.info.offset, 5);
    }

    let found = gpiocdev::find_named_lines(&["fls apple"], true);
    assert_eq!(
        found,
        Err(gpiocdev::Error::NonuniqueLineName("fls apple".to_string()))
    );

    let found = gpiocdev::find_named_lines(&["fls banana", "fls piggly"], true).unwrap();
    assert_eq!(found.len(), 2);

    let found = gpiocdev::find_named_lines(&["fls nada"], true).unwrap();
    assert_eq!(found.len(), 0);

    let found =
        gpiocdev::find_named_lines(&["fls apple", "fls banana", "fls nada"], false).unwrap();
    let l = found.get(&"fls banana").unwrap();
    assert_eq!(&l.chip, sim.chips()[0].dev_path());
    assert_eq!(l.info.offset, 3);
    let l = found.get(&"fls apple").unwrap();
    if compare(
        &sim.chips()[0].dev_path().to_string_lossy(),
        &sim.chips()[1].dev_path().to_string_lossy(),
    ) == Ordering::Less
    {
        assert_eq!(&l.chip, sim.chips()[0].dev_path());
        assert_eq!(l.info.offset, 6);
    } else {
        assert_eq!(&l.chip, sim.chips()[1].dev_path());
        assert_eq!(l.info.offset, 5);
    }
    assert!(found.get(&"fls nada").is_none());
}

#[test]
fn detect_abi_version() {
    // assumes a kernel with both v1 and v2 supported.

    // can't test NoChips case, as cannot guarantee no chips
    // (even if we check there may be other tests creating them concurrently)

    // to ensure there is at least one chip
    let s = gpiosim::Simpleton::new(4);

    #[cfg(feature = "uapi_v2")]
    assert_eq!(gpiocdev::detect_abi_version(), Ok(gpiocdev::AbiVersion::V2));
    #[cfg(not(feature = "uapi_v2"))]
    assert_eq!(gpiocdev::detect_abi_version(), Ok(gpiocdev::AbiVersion::V1));

    drop(s);
}

#[test]
fn supports_abi_version() {
    // assumes a kernel with both v1 and v2 supported.

    // to ensure there is at least one chip
    let s = gpiosim::Simpleton::new(4);

    #[cfg(feature = "uapi_v1")]
    assert_eq!(
        gpiocdev::supports_abi_version(gpiocdev::AbiVersion::V1),
        Ok(())
    );
    #[cfg(not(feature = "uapi_v1"))]
    assert_eq!(
        gpiocdev::supports_abi_version(gpiocdev::AbiVersion::V1),
        Err(gpiocdev::Error::UnsupportedAbi(
            gpiocdev::AbiVersion::V1,
            gpiocdev::AbiSupportKind::Build
        ))
    );
    #[cfg(feature = "uapi_v2")]
    assert_eq!(
        gpiocdev::supports_abi_version(gpiocdev::AbiVersion::V2),
        Ok(())
    );
    #[cfg(not(feature = "uapi_v2"))]
    assert_eq!(
        gpiocdev::supports_abi_version(gpiocdev::AbiVersion::V2),
        Err(gpiocdev::Error::UnsupportedAbi(
            gpiocdev::AbiVersion::V2,
            gpiocdev::AbiSupportKind::Build
        ))
    );

    drop(s);
}
