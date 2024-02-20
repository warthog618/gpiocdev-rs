// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use criterion::{criterion_group, criterion_main, Criterion};
use gpiosim::{Level, Simpleton};

// determine overhead from toggling sim lines
fn toggle_line(c: &mut Criterion) {
    let s = Simpleton::new(10);
    let offset = 1;

    let mut pull = Level::High;

    c.bench_function("toggle_line", |b| {
        b.iter(|| {
            s.set_pull(offset, pull).unwrap();
            pull = match pull {
                Level::High => Level::Low,
                Level::Low => Level::High,
            };
        })
    });
}

criterion_group!(benches, toggle_line);
criterion_main!(benches);
