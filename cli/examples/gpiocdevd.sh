#!/bin/bash
# SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
#
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Example of using the gpiocdev set --interactive mode to create a simple GPIO daemon.
# 
# Other programs can drive the GPIO by writing commands to the pipe,
# e.g.
#
# echo toggle > /tmp/gpiocdevd
#
# or
#
# echo "set GPIO23=1" > /tmp/gpiocdevd
#
# similar to setting with the deprecated sysfs interface.

pipe=/tmp/gpiocdevd

mkfifo $pipe

trap "rm -f $pipe" EXIT

# as bash will block until something is written to the pipe...
echo "" > $pipe &
gpiocdev set -i GPIO23=0 < $pipe > /dev/null
