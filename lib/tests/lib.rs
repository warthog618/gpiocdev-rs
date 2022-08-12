// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

// ALL IT as can't construct a Chip without opening a GPIO file.
//
// Assumptions:
//  - kernel supports uAPI versions corresponding to selected build features

#[test]
fn detect_abi_version() {
    // assumes a kernel with both v1 and v2 supported.

    // to ensure there is at least one chip
    let sim = gpiosim::simpleton(4);

    #[cfg(feature = "uapi_v2")]
    assert_eq!(gpiocdev::detect_abi_version(), Ok(gpiocdev::AbiVersion::V2));
    #[cfg(not(feature = "uapi_v2"))]
    assert_eq!(gpiocdev::detect_abi_version(), Ok(gpiocdev::AbiVersion::V1));

    drop(sim);
}

#[test]
fn supports_abi_version() {
    // assumes a kernel with both v1 and v2 supported.

    // to ensure there is at least one chip
    let sim = gpiosim::simpleton(4);

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
            gpiocdev::AbiSupportKind::Library
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
            gpiocdev::AbiSupportKind::Library
        ))
    );

    drop(sim);
}
