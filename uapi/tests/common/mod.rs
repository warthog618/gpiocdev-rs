// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::Duration;

// max time to wait for an event - expected or not
pub const EVENT_WAIT_TIMEOUT: Duration = Duration::from_millis(25);

// max time to allow events to propagate fromn the sim to cdev
const PROPAGATION_DELAY: Duration = Duration::from_millis(10);

pub fn wait_propagation_delay() {
    std::thread::sleep(PROPAGATION_DELAY);
}

// a collection of chips with named and hogged lines for find and info tests
pub fn detailed_sim() -> gpiosim::Sim {
    use gpiosim::{Bank, Direction};
    gpiosim::builder()
        .with_bank(
            Bank::new(8, "fruit")
                .name(3, "banana")
                .name(5, "apple")
                .name(7, "pear")
                .hog(2, "bowl", Direction::OutputLow)
                .hog(5, "pie", Direction::Input),
        )
        .with_bank(
            Bank::new(41, "vegetables")
                .name(3, "arugula")
                .name(4, "celery")
                .name(6, "pumpkin")
                .hog(3, "salad", Direction::OutputHigh),
        )
        .live()
        .expect("gpiosim should go live")
}
