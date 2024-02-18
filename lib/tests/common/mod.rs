// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::Duration;

// max time to allow events to propagate from the sim to cdev
const PROPAGATION_DELAY: Duration = Duration::from_millis(10);

pub fn wait_propagation_delay() {
    std::thread::sleep(PROPAGATION_DELAY);
}
