#!/bin/env sh
# SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
#
# SPDX-License-Identifier: Apache-2.0 OR MIT

# A helper to remove any orphaned gpio-sims from the system.
# This should only be necessary if a test was killed abnormally
# preventing it from cleaning up the sims it created.

ls -d /sys/kernel/config/gpio-sim/*/*/*/hog 2>/dev/null | xargs -r rmdir
ls -d /sys/kernel/config/gpio-sim/*/*/line* 2>/dev/null | xargs -r rmdir
ls -d /sys/kernel/config/gpio-sim/*/bank* 2>/dev/null | xargs -r rmdir
ls -d /sys/kernel/config/gpio-sim/* 2>/dev/null | xargs -r rmdir

