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
        get_line_event, get_line_handle, EventRequest, EventRequestFlags, HandleRequest,
        HandleRequestFlags, Offset,
    };
    use gpiosim::Simpleton;
    use std::fs;

    pub fn bench(c: &mut Criterion) {
        c.bench_function(
            "uapi_v1 open chip and request one",
            open_chip_and_request_one,
        );
        c.bench_function("uapi_v1 request one", request_one);
        c.bench_function("uapi_v1 request event", request_event);
        c.bench_function("uapi_v1 request ten", request_ten);
        c.bench_function("uapi_v1 request maxlen", request_maxlen);
    }

    // determine time taken to open a gpiochip and request one line
    fn open_chip_and_request_one(b: &mut Bencher) {
        let s = Simpleton::new(10);
        b.iter(|| {
            let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
            let mut hr = HandleRequest {
                num_lines: 1,
                consumer: "open_chip_and_request_one".into(),
                flags: HandleRequestFlags::INPUT,
                ..Default::default()
            };
            // doesn't have to be in order, but just keeping it simple...
            hr.offsets.copy_from_slice(&[1]);

            let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");
            drop(l);
            drop(cf);
        });
    }
    // determine time taken to request one line
    fn request_one(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        b.iter(|| {
            let mut hr = HandleRequest {
                num_lines: 1,
                consumer: "request_one".into(),
                flags: HandleRequestFlags::INPUT,
                ..Default::default()
            };
            // doesn't have to be in order, but just keeping it simple...
            hr.offsets.copy_from_slice(&[1]);

            let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");
            drop(l);
        });
    }
    // determine time taken to request a line with edge detection
    fn request_event(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        b.iter(|| {
            let er = EventRequest {
                offset: 1,
                consumer: "request_event".into(),
                eventflags: EventRequestFlags::BOTH_EDGES,
                ..Default::default()
            };

            let l = get_line_event(&cf, er).expect("get_line_event should succeed");
            drop(l);
        });
    }

    // determine time taken to requst ten lines
    fn request_ten(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        b.iter(|| {
            let mut hr = HandleRequest {
                num_lines: 10,
                consumer: "request_ten".into(),
                flags: HandleRequestFlags::INPUT,
                ..Default::default()
            };
            // doesn't have to be in order, but just keeping it simple...
            let offsets: Vec<Offset> = (0..10).collect();
            hr.offsets.copy_from_slice(&offsets);

            let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");
            drop(l);
        });
    }

    // determine time taken to request the maxiumum number of lines
    fn request_maxlen(b: &mut Bencher) {
        let s = Simpleton::new(64);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        b.iter(|| {
            let mut hr = HandleRequest {
                num_lines: 64,
                consumer: "request_maxlen".into(),
                flags: HandleRequestFlags::INPUT,
                ..Default::default()
            };
            // doesn't have to be in order, but just keeping it simple...
            let offsets: Vec<Offset> = (0..64).collect();
            hr.offsets.copy_from_slice(&offsets);

            let l = get_line_handle(&cf, hr).expect("get_line_handle should succeed");
            drop(l);
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
    use gpiocdev_uapi::v2::{get_line, LineAttribute, LineConfig, LineFlags, LineRequest, Offset};
    use gpiosim::Simpleton;
    use std::fs;

    pub fn bench(c: &mut Criterion) {
        c.bench_function(
            "uapi_v2 open chip and request one",
            open_chip_and_request_one,
        );
        c.bench_function("uapi_v2 request one", request_one);
        c.bench_function(
            "uapi_v2 request one with both edges",
            request_one_with_both_edges,
        );
        c.bench_function(
            "uapi_v2 request one with both edges debounced",
            request_one_with_both_edges_debounced,
        );
        c.bench_function("uapi_v2 request ten", request_ten);
        c.bench_function("uapi_v2 request maxlen", request_maxlen);
    }

    // determine time taken to open a gpio chip and request one line
    fn open_chip_and_request_one(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let offset = 2;
        b.iter(|| {
            let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
            let mut lr = LineRequest {
                num_lines: 1,
                consumer: "open_chip_and_request_one".into(),
                config: LineConfig {
                    flags: LineFlags::INPUT,
                    ..Default::default()
                },
                ..Default::default()
            };
            // doesn't have to be in order, but just keeping it simple...
            lr.offsets.copy_from_slice(&[offset]);

            let l = get_line(&cf, lr).expect("get_line should succeed");
            drop(l);
            drop(cf);
        });
    }
    // determine time taken to request one line
    fn request_one(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let offset = 2;
        b.iter(|| {
            let mut lr = LineRequest {
                num_lines: 1,
                consumer: "request_one".into(),
                config: LineConfig {
                    flags: LineFlags::INPUT,
                    ..Default::default()
                },
                ..Default::default()
            };
            // doesn't have to be in order, but just keeping it simple...
            lr.offsets.copy_from_slice(&[offset]);

            let l = get_line(&cf, lr).expect("get_line should succeed");
            drop(l);
        });
    }

    // determine time taken to request one line with edge detection
    fn request_one_with_both_edges(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let offset = 2;
        b.iter(|| {
            let mut lr = LineRequest {
                num_lines: 1,
                consumer: "request_one".into(),
                config: LineConfig {
                    flags: LineFlags::INPUT | LineFlags::EDGE_RISING | LineFlags::EDGE_FALLING,
                    ..Default::default()
                },
                ..Default::default()
            };
            // doesn't have to be in order, but just keeping it simple...
            lr.offsets.copy_from_slice(&[offset]);

            let l = get_line(&cf, lr).expect("get_line should succeed");
            drop(l);
        });
    }

    // determine time taken to request one line with edge detection and debounce
    fn request_one_with_both_edges_debounced(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let offset = 2;
        b.iter(|| {
            let mut lr = LineRequest {
                num_lines: 1,
                consumer: "request_one".into(),
                config: LineConfig {
                    flags: LineFlags::INPUT | LineFlags::EDGE_RISING | LineFlags::EDGE_FALLING,
                    num_attrs: 1,
                    ..Default::default()
                },
                ..Default::default()
            };
            // doesn't have to be in order, but just keeping it simple...
            lr.offsets.copy_from_slice(&[offset]);
            let mut xattr = LineAttribute::default();
            xattr.set_debounce_period_us(123);
            let attr = lr.config.attr_mut(0);
            attr.mask = 1;
            attr.attr = xattr;

            let l = get_line(&cf, lr).expect("get_line should succeed");
            drop(l);
        });
    }

    // determine time taken to request ten lines
    fn request_ten(b: &mut Bencher) {
        let s = Simpleton::new(10);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        b.iter(|| {
            let mut lr = LineRequest {
                num_lines: 10,
                consumer: "request_ten".into(),
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
            drop(l);
        });
    }

    // determine time taken to request maximum number of lines
    fn request_maxlen(b: &mut Bencher) {
        let s = Simpleton::new(64);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        b.iter(|| {
            let mut lr = LineRequest {
                num_lines: 64,
                consumer: "request_maxlen".into(),
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
            drop(l);
        });
    }
}
#[cfg(not(feature = "uapi_v2"))]
mod v2 {
    pub fn bench(_c: &mut criterion::Criterion) {}
}
