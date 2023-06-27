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
    c.bench_function("uapi_v1 one input", |b| one_input(b, V1));
    c.bench_function("uapi_v1 ten inputs", |b| ten_inputs(b, V1));
    c.bench_function("uapi_v1 one output", |b| one_output(b, V1));
    c.bench_function("uapi_v1 ten outputs", |b| ten_outputs(b, V1));
}
#[cfg(not(feature = "uapi_v1"))]
fn v1_benchmarks(_c: &mut Criterion) {}

#[cfg(feature = "uapi_v2")]
fn v2_benchmarks(c: &mut Criterion) {
    use gpiocdev::AbiVersion::V2;
    c.bench_function("uapi_v2 one input", |b| one_input(b, V2));
    c.bench_function("uapi_v2 ten inputs", |b| ten_inputs(b, V2));
    c.bench_function("uapi_v2 one output", |b| one_output(b, V2));
    c.bench_function("uapi_v2 ten outputs", |b| ten_outputs(b, V2));
}
#[cfg(not(feature = "uapi_v2"))]
fn v2_benchmarks(_c: &mut Criterion) {}

#[allow(unused_variables)]
fn one_input(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(10);
    let offset = 1;

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);

    b.iter(|| {
        let req = builder.with_line(offset).as_input().request().unwrap();
        drop(req);
    });
}

#[allow(unused_variables)]
fn ten_inputs(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(10);
    let offsets = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);

    b.iter(|| {
        let req = builder.with_lines(&offsets).as_input().request().unwrap();
        drop(req);
    });
}

#[allow(unused_variables)]
fn one_output(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(10);
    let offset = 1;

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);

    b.iter(|| {
        let req = builder
            .with_line(offset)
            .as_output(Value::Inactive)
            .request()
            .unwrap();
        drop(req);
    });
}

#[allow(unused_variables)]
fn ten_outputs(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(10);
    let offsets = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);

    b.iter(|| {
        let req = builder
            .with_lines(&offsets)
            .as_output(Value::Inactive)
            .request()
            .unwrap();
        drop(req);
    });
}
