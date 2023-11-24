// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use criterion::{criterion_group, criterion_main, Bencher, Criterion};

use gpiocdev::line::Value;
use gpiocdev::request::Request;
use gpiocdev::AbiVersion;
use gpiosim::Simpleton;

criterion_group!(v1_benches, v1_benchmarks);
criterion_group!(v2_benches, v2_benchmarks);
criterion_main!(v1_benches, v2_benches);

#[cfg(feature = "uapi_v1")]
fn v1_benchmarks(c: &mut Criterion) {
    use gpiocdev::AbiVersion::V1;
    c.bench_function("uapi_v1 reconfigure input", |b| reconfigure_input(b, V1));
    c.bench_function("uapi_v1 reconfigure output", |b| reconfigure_output(b, V1));
    c.bench_function("uapi_v1 reconfigure input and output", |b| reconfigure_input_output(b, V1));
}
#[cfg(not(feature = "uapi_v1"))]
fn v1_benchmarks(_c: &mut Criterion) {}

#[cfg(feature = "uapi_v2")]
fn v2_benchmarks(c: &mut Criterion) {
    use gpiocdev::AbiVersion::V2;
    c.bench_function("uapi_v2 reconfigure input", |b| reconfigure_input(b, V2));
    c.bench_function("uapi_v2 reconfigure output", |b| reconfigure_output(b, V2));
    c.bench_function("uapi_v2 reconfigure input and output", |b| reconfigure_input_output(b, V2));
}
#[cfg(not(feature = "uapi_v2"))]
fn v2_benchmarks(_c: &mut Criterion) {}

// determine time taken to reconfigure as input
#[allow(unused_variables)]
fn reconfigure_input(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(4);
    let offset = 1;

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);
    let req = builder.with_line(offset).as_output(Value::Active).request().unwrap();
    let mut cfg = req.config();
    cfg.as_input();

    b.iter(|| {
        req.reconfigure(&cfg).unwrap();
    });
}

// determine time taken to reconfigure as output
#[allow(unused_variables)]
fn reconfigure_output(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(4);
    let offset = 1;

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);
    let req = builder.with_line(offset).as_input().request().unwrap();
    let mut cfg = req.config();
    cfg.as_output(Value::Active);

    b.iter(|| {
        req.reconfigure(&cfg).unwrap();
    });
}

// determine time taken to reconfigure to input then output
// (in case the device driver is short circuiting when reconfiguring to the current state)
#[allow(unused_variables)]
fn reconfigure_input_output(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(4);
    let offset = 1;

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);
    let req = builder.with_line(offset).as_input().request().unwrap();
    let mut in_cfg = req.config();
    in_cfg.as_input();
    let mut out_cfg = req.config();
    out_cfg.as_output(Value::Active);

    b.iter(|| {
        req.reconfigure(&out_cfg).unwrap();
        req.reconfigure(&in_cfg).unwrap();
    });
}
