// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use criterion::{criterion_group, criterion_main};

criterion_group!(v1_benches, v1::bench);
criterion_group!(v2_benches, v2::bench);
criterion_main!(v1_benches, v2_benches);

#[cfg(feature = "uapi_v1")]
mod v1 {
    use criterion::{Bencher, Criterion};
    use gpiocdev_uapi::v1::{
        get_line_handle, set_line_config, HandleConfig, HandleRequest, HandleRequestFlags,
    };
    use gpiosim::Simpleton;
    use std::fs;

    pub fn bench(c: &mut Criterion) {
        c.bench_function("uapi_v1 reconfigure input", reconfigure_input);
        c.bench_function("uapi_v1 reconfigure output", reconfigure_output);
        c.bench_function(
            "uapi_v1 reconfigure input and output",
            reconfigure_input_output,
        );
    }

    // determine time taken to reconfigure one line to input
    fn reconfigure_input(b: &mut Bencher) {
        let s = Simpleton::new(4);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut hr = HandleRequest {
            num_lines: 1,
            consumer: "reconfigure_input".into(),
            flags: HandleRequestFlags::OUTPUT,
            ..Default::default()
        };
        hr.offsets.copy_from_slice(&[1]);

        let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");

        b.iter(|| {
            let hc = HandleConfig {
                flags: HandleRequestFlags::INPUT,
                ..Default::default()
            };

            set_line_config(&l, hc).expect("set_line_config should succeed");
        });
    }

    // determine time taken to reconfigure one line to output
    fn reconfigure_output(b: &mut Bencher) {
        let s = Simpleton::new(4);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut hr = HandleRequest {
            num_lines: 1,
            consumer: "reconfigure_output".into(),
            flags: HandleRequestFlags::INPUT,
            ..Default::default()
        };
        hr.offsets.copy_from_slice(&[1]);

        let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");

        b.iter(|| {
            let hc = HandleConfig {
                flags: HandleRequestFlags::OUTPUT,
                ..Default::default()
            };

            set_line_config(&l, hc).expect("set_line_config should succeed");
        });
    }

    // determine time taken to reconfigure one line to input then output
    fn reconfigure_input_output(b: &mut Bencher) {
        let s = Simpleton::new(4);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut hr = HandleRequest {
            num_lines: 1,
            consumer: "reconfigure_input_output".into(),
            flags: HandleRequestFlags::INPUT,
            ..Default::default()
        };
        hr.offsets.copy_from_slice(&[1]);

        let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");

        b.iter(|| {
            let hc = HandleConfig {
                flags: HandleRequestFlags::OUTPUT,
                ..Default::default()
            };

            set_line_config(&l, hc).expect("set_line_config should succeed");

            let hc = HandleConfig {
                flags: HandleRequestFlags::INPUT,
                ..Default::default()
            };

            set_line_config(&l, hc).expect("set_line_config should succeed");
        });
    }
}
#[cfg(not(feature = "uapi_v1"))]
mod v1 {
    pub fn bench(_c: &mut criterion::Criterion) {}
}

#[cfg(feature = "uapi_v2")]
mod v2 {
    use criterion::{Bencher, Criterion};
    use gpiocdev_uapi::v2::{get_line, set_line_config, LineConfig, LineFlags, LineRequest};
    use gpiosim::Simpleton;
    use std::fs;

    pub fn bench(c: &mut Criterion) {
        c.bench_function("uapi_v2 reconfigure input", reconfigure_input);
        c.bench_function("uapi_v2 reconfigure output", reconfigure_output);
        c.bench_function(
            "uapi_v2 reconfigure input and output",
            reconfigure_input_output,
        );
    }

    // determine time taken to reconfigure one line to input
    fn reconfigure_input(b: &mut Bencher) {
        let s = Simpleton::new(4);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut lr = LineRequest {
            num_lines: 1,
            consumer: "reconfigure_input".into(),
            config: LineConfig {
                flags: LineFlags::OUTPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        lr.offsets.copy_from_slice(&[1]);

        let l = get_line(&cf, lr).expect("get_line should succeed");

        b.iter(|| {
            let lc = LineConfig {
                flags: LineFlags::INPUT,
                ..Default::default()
            };

            set_line_config(&l, lc).expect("set_line_config should succeed");
        });
    }

    // determine time taken to reconfigure one line to output
    fn reconfigure_output(b: &mut Bencher) {
        let s = Simpleton::new(4);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut lr = LineRequest {
            num_lines: 1,
            consumer: "reconfigure_output".into(),
            config: LineConfig {
                flags: LineFlags::INPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        lr.offsets.copy_from_slice(&[1]);

        let l = get_line(&cf, lr).expect("get_line should succeed");

        b.iter(|| {
            let lc = LineConfig {
                flags: LineFlags::OUTPUT,
                ..Default::default()
            };

            set_line_config(&l, lc).expect("set_line_config should succeed");
        });
    }

    // determine time taken to reconfigure one line to input then output
    fn reconfigure_input_output(b: &mut Bencher) {
        let s = Simpleton::new(4);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut lr = LineRequest {
            num_lines: 1,
            consumer: "reconfigure_output".into(),
            config: LineConfig {
                flags: LineFlags::INPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        lr.offsets.copy_from_slice(&[1]);

        let l = get_line(&cf, lr).expect("get_line should succeed");

        b.iter(|| {
            let lc = LineConfig {
                flags: LineFlags::OUTPUT,
                ..Default::default()
            };

            set_line_config(&l, lc).expect("set_line_config should succeed");

            let lc = LineConfig {
                flags: LineFlags::INPUT,
                ..Default::default()
            };

            set_line_config(&l, lc).expect("set_line_config should succeed");
        });
    }
}
#[cfg(not(feature = "uapi_v2"))]
mod v2 {
    pub fn bench(_c: &mut criterion::Criterion) {}
}
