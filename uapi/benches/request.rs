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
        LineValues,
    };
    use gpiosim::Simpleton;
    use std::fs;
    use std::os::unix::prelude::AsRawFd;

    pub fn bench(c: &mut Criterion) {
        c.bench_function("uapi_v1 get one", get_one);
        c.bench_function("uapi_v1 get ten", get_ten);
        c.bench_function("uapi_v1 set one", set_one);
        c.bench_function("uapi_v1 set ten", set_ten);
    }

    // determine time taken to get one line
    fn get_one(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let f = fs::File::open(s.dev_path()).unwrap();
        let cfd = f.as_raw_fd();
        let mut hr = HandleRequest {
            num_lines: 1,
            consumer: "get_one".into(),
            flags: HandleRequestFlags::INPUT,
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        hr.offsets.copy_from_slice(&[1]);

        let l = get_line_handle(cfd, hr).unwrap();
        let lfd = l.as_raw_fd();

        // sim defaults to pulling low
        let mut values = LineValues::default();

        b.iter(|| {
            get_line_values(lfd, &mut values).unwrap();
        });
    }

    // determine time taken to get ten lines
    fn get_ten(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let f = fs::File::open(s.dev_path()).unwrap();
        let cfd = f.as_raw_fd();
        let mut hr = HandleRequest {
            num_lines: 10,
            consumer: "get_ten".into(),
            flags: HandleRequestFlags::INPUT,
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        hr.offsets.copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);

        let l = get_line_handle(cfd, hr).unwrap();
        let lfd = l.as_raw_fd();

        // sim defaults to pulling low
        let mut values = LineValues::default();

        b.iter(|| {
            get_line_values(lfd, &mut values).unwrap();
        });
    }

    // determine time taken to set one line
    fn set_one(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let f = fs::File::open(s.dev_path()).unwrap();
        let cfd = f.as_raw_fd();
        let mut hr = HandleRequest {
            num_lines: 1,
            consumer: "set_one".into(),
            flags: HandleRequestFlags::OUTPUT,
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        hr.offsets.copy_from_slice(&[1]);

        let l = get_line_handle(cfd, hr).unwrap();
        let lfd = l.as_raw_fd();

        // sim defaults to pulling low
        let values = LineValues::default();

        b.iter(|| {
            set_line_values(lfd, &values).unwrap();
        });
    }

    // determine time taken to set multiple lines
    fn set_ten(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let f = fs::File::open(s.dev_path()).unwrap();
        let cfd = f.as_raw_fd();
        let mut hr = HandleRequest {
            num_lines: 10,
            consumer: "set_ten".into(),
            flags: HandleRequestFlags::OUTPUT,
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        hr.offsets.copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);

        let l = get_line_handle(cfd, hr).unwrap();
        let lfd = l.as_raw_fd();

        // sim defaults to pulling low
        let values = LineValues::default();

        b.iter(|| {
            set_line_values(lfd, &values).unwrap();
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
    };
    use gpiosim::Simpleton;
    use std::fs;
    use std::os::unix::prelude::AsRawFd;

    pub fn bench(c: &mut Criterion) {
        c.bench_function("uapi_v2 get one", get_one);
        c.bench_function("uapi_v2 get ten", get_ten);
        c.bench_function("uapi_v2 set one", set_one);
        c.bench_function("uapi_v2 set ten", set_ten);
    }

    // determine time taken to get one line
    fn get_one(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let f = fs::File::open(s.dev_path()).unwrap();
        let cfd = f.as_raw_fd();
        let offset = 2;
        let mut lr = LineRequest {
            num_lines: 1,
            consumer: "get_one".into(),
            config: LineConfig {
                flags: LineFlags::OUTPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        lr.offsets.copy_from_slice(&[offset]);

        let l = get_line(cfd, lr).unwrap();
        let lfd = l.as_raw_fd();

        // sim defaults to pulling low
        let mut values = LineValues::from_slice(&[true]);

        b.iter(|| {
            get_line_values(lfd, &mut values).unwrap();
        });
    }

    // determine time taken to get ten lines
    fn get_ten(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let f = fs::File::open(s.dev_path()).unwrap();
        let cfd = f.as_raw_fd();
        let mut hr = LineRequest {
            num_lines: 10,
            consumer: "get_ten".into(),
            config: LineConfig {
                flags: LineFlags::OUTPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        hr.offsets.copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);

        let l = get_line(cfd, hr).unwrap();
        let lfd = l.as_raw_fd();

        // sim defaults to pulling low
        let mut values = LineValues::from_slice(&[false; 10]);

        b.iter(|| {
            get_line_values(lfd, &mut values).unwrap();
        });
    }

    // determine time taken to set one line
    fn set_one(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let f = fs::File::open(s.dev_path()).unwrap();
        let cfd = f.as_raw_fd();
        let offset = 2;
        let mut hr = LineRequest {
            num_lines: 1,
            consumer: "set_one".into(),
            config: LineConfig {
                flags: LineFlags::OUTPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        hr.offsets.copy_from_slice(&[offset]);

        let l = get_line(cfd, hr).unwrap();
        let lfd = l.as_raw_fd();

        // sim defaults to pulling low
        let values = LineValues::from_slice(&[true]);

        b.iter(|| {
            set_line_values(lfd, &values).unwrap();
        });
    }

    // determine time taken to set multiple lines
    fn set_ten(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let f = fs::File::open(s.dev_path()).unwrap();
        let cfd = f.as_raw_fd();
        let mut hr = LineRequest {
            num_lines: 10,
            consumer: "set_ten".into(),
            config: LineConfig {
                flags: LineFlags::OUTPUT,
                ..Default::default()
            },
            ..Default::default()
        };
        // doesn't have to be in order, but just keeping it simple...
        hr.offsets.copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);

        let l = get_line(cfd, hr).unwrap();
        let lfd = l.as_raw_fd();

        let values = LineValues::from_slice(&[true; 10]);

        b.iter(|| {
            set_line_values(lfd, &values).unwrap();
        });
    }
}
#[cfg(not(feature = "uapi_v2"))]
mod v2 {
    pub fn bench(_c: &mut criterion::Criterion) {}
}
