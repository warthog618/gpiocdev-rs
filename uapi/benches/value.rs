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
        get_line_handle, get_line_values, set_line_values, HandleRequest, HandleRequestFlags,
        LineValues, Offset,
    };
    use gpiosim::Simpleton;
    use std::fs;

    pub fn bench(c: &mut Criterion) {
        c.bench_function("uapi_v1 get one", get_one);
        c.bench_function("uapi_v1 get ten", get_ten);
        c.bench_function("uapi_v1 get maxlen", get_maxlen);
        c.bench_function("uapi_v1 set one", set_one);
        c.bench_function("uapi_v1 set ten", set_ten);
        c.bench_function("uapi_v1 set maxlen", set_maxlen);
    }

    // determine time taken to get one line
    fn get_one(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut hr = HandleRequest {
            num_lines: 1,
            consumer: "get_one".into(),
            flags: HandleRequestFlags::INPUT,
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        hr.offsets.copy_from_slice(&[1]);

        let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");

        // sim defaults to pulling low
        let mut values = LineValues::default();

        b.iter(|| {
            get_line_values(&l, &mut values).expect("get_line_values should succeed");
        });
    }

    // determine time taken to get ten lines
    fn get_ten(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut hr = HandleRequest {
            num_lines: 10,
            consumer: "get_ten".into(),
            flags: HandleRequestFlags::INPUT,
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        let offsets: Vec<Offset> = (0..10).collect();
        hr.offsets.copy_from_slice(&offsets);

        let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");

        // sim defaults to pulling low
        let mut values = LineValues::default();

        b.iter(|| {
            get_line_values(&l, &mut values).expect("get_line_values should succeed");
        });
    }

    // determine time taken to get the maximum number of lines
    fn get_maxlen(b: &mut Bencher) {
        let s = Simpleton::new(64);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut hr = HandleRequest {
            num_lines: 64,
            consumer: "get_ten".into(),
            flags: HandleRequestFlags::INPUT,
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        let offsets: Vec<Offset> = (0..64).collect();
        hr.offsets.copy_from_slice(&offsets);

        let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");

        // sim defaults to pulling low
        let mut values = LineValues::default();

        b.iter(|| {
            get_line_values(&l, &mut values).expect("get_line_values should succeed");
        });
    }

    // determine time taken to set one line
    fn set_one(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut hr = HandleRequest {
            num_lines: 1,
            consumer: "set_one".into(),
            flags: HandleRequestFlags::OUTPUT,
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        hr.offsets.copy_from_slice(&[1]);

        let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");

        // sim defaults to pulling low
        let values = LineValues::default();

        b.iter(|| {
            set_line_values(&l, &values).expect("set_line_values should succeed");
        });
    }

    // determine time taken to set ten lines
    fn set_ten(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut hr = HandleRequest {
            num_lines: 10,
            consumer: "set_ten".into(),
            flags: HandleRequestFlags::OUTPUT,
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        let offsets: Vec<Offset> = (0..10).collect();
        hr.offsets.copy_from_slice(&offsets);

        let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");

        // sim defaults to pulling low
        let values = LineValues::default();

        b.iter(|| {
            set_line_values(&l, &values).expect("set_line_values should succeed");
        });
    }

    // determine time taken to set the maximum number of lines
    fn set_maxlen(b: &mut Bencher) {
        let s = Simpleton::new(64);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut hr = HandleRequest {
            num_lines: 64,
            consumer: "set_maxlen".into(),
            flags: HandleRequestFlags::OUTPUT,
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        let offsets: Vec<Offset> = (0..64).collect();
        hr.offsets.copy_from_slice(&offsets);

        let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");

        // sim defaults to pulling low
        let values = LineValues::default();

        b.iter(|| {
            set_line_values(&l, &values).expect("set_line_values should succeed");
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
    use gpiocdev_uapi::v2::{
        get_line, get_line_values, set_line_values, LineConfig, LineFlags, LineRequest, LineValues,
        Offset,
    };
    use gpiosim::Simpleton;
    use std::fs;

    pub fn bench(c: &mut Criterion) {
        c.bench_function("uapi_v2 get one", get_one);
        c.bench_function("uapi_v2 get ten", get_ten);
        c.bench_function("uapi_v2 get maxlen", get_maxlen);
        c.bench_function("uapi_v2 set one", set_one);
        c.bench_function("uapi_v2 set ten", set_ten);
        c.bench_function("uapi_v2 set maxlen", set_maxlen);
    }

    // determine time taken to get one line
    fn get_one(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let offset = 2;
        let mut lr = LineRequest {
            num_lines: 1,
            consumer: "get_one".into(),
            config: LineConfig {
                flags: LineFlags::INPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        lr.offsets.copy_from_slice(&[offset]);

        let l = get_line(&cf, lr).expect("get_line should succeed");

        // sim defaults to pulling low
        let mut values = LineValues::from_slice(&[true]);

        b.iter(|| {
            get_line_values(&l, &mut values).expect("get_line_values should succeed");
        });
    }

    // determine time taken to get ten lines
    fn get_ten(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut lr = LineRequest {
            num_lines: 10,
            consumer: "get_ten".into(),
            config: LineConfig {
                flags: LineFlags::INPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        let offsets: Vec<Offset> = (0..10).collect();
        lr.offsets.copy_from_slice(&offsets);

        let l = get_line(&cf, lr).expect("get_line should succeed");

        let mut values = LineValues::from_slice(&[false; 10]);

        b.iter(|| {
            get_line_values(&l, &mut values).expect("get_line_values should succeed");
        });
    }

    // determine time taken to get the maximum number of lines
    fn get_maxlen(b: &mut Bencher) {
        let s = Simpleton::new(64);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut lr = LineRequest {
            num_lines: 64,
            consumer: "get_maxlen".into(),
            config: LineConfig {
                flags: LineFlags::OUTPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        let offsets: Vec<Offset> = (0..64).collect();
        lr.offsets.copy_from_slice(&offsets);

        let l = get_line(&cf, lr).expect("get_line should succeed");

        let mut values = LineValues::from_slice(&[false; 64]);

        b.iter(|| {
            get_line_values(&l, &mut values).expect("get_line_values should succeed");
        });
    }

    // determine time taken to set one line
    fn set_one(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let offset = 2;
        let mut lr = LineRequest {
            num_lines: 1,
            consumer: "set_one".into(),
            config: LineConfig {
                flags: LineFlags::OUTPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        lr.offsets.copy_from_slice(&[offset]);

        let l = get_line(&cf, lr).expect("get_line should succeed");

        // sim defaults to pulling low
        let values = LineValues::from_slice(&[true]);

        b.iter(|| {
            set_line_values(&l, &values).expect("set_line_values should succeed");
        });
    }

    // determine time taken to set ten lines
    fn set_ten(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut lr = LineRequest {
            num_lines: 10,
            consumer: "set_ten".into(),
            config: LineConfig {
                flags: LineFlags::OUTPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        let offsets: Vec<Offset> = (0..10).collect();
        lr.offsets.copy_from_slice(&offsets);

        let l = get_line(&cf, lr).expect("get_line should succeed");

        let values = LineValues::from_slice(&[true; 10]);

        b.iter(|| {
            set_line_values(&l, &values).expect("set_line_values should succeed");
        });
    }

    // determine time taken to set the maximum number of lines
    fn set_maxlen(b: &mut Bencher) {
        let s = Simpleton::new(64);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let mut lr = LineRequest {
            num_lines: 64,
            consumer: "set_maxlen".into(),
            config: LineConfig {
                flags: LineFlags::OUTPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        let offsets: Vec<Offset> = (0..64).collect();
        lr.offsets.copy_from_slice(&offsets);

        let l = get_line(&cf, lr).expect("get_line should succeed");

        let values = LineValues::from_slice(&[true; 64]);

        b.iter(|| {
            set_line_values(&l, &values).expect("set_line_values should succeed");
        });
    }
}
#[cfg(not(feature = "uapi_v2"))]
mod v2 {
    pub fn bench(_c: &mut criterion::Criterion) {}
}
