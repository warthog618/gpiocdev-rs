// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use criterion::{criterion_group, criterion_main, Bencher, Criterion};

use gpiocdev::line::{Offset, Value, Values};
use gpiocdev::request::Request;
use gpiocdev::AbiVersion;
use gpiosim::Simpleton;

criterion_group!(v1_benches, v1_benchmarks);
criterion_group!(v2_benches, v2_benchmarks);
criterion_main!(v1_benches, v2_benches);

#[cfg(feature = "uapi_v1")]
fn v1_benchmarks(c: &mut Criterion) {
    use gpiocdev::AbiVersion::V1;
    c.bench_function("uapi_v1 get one", |b| get_one(b, V1));
    c.bench_function("uapi_v1 get ten", |b| get_ten(b, V1));
    c.bench_function("uapi_v1 get maxlen", |b| get_maxlen(b, V1));
    c.bench_function("uapi_v1 set one", |b| set_one(b, V1));
    c.bench_function("uapi_v1 set ten", |b| set_ten(b, V1));
    c.bench_function("uapi_v1 set maxlen", |b| set_maxlen(b, V1));
}
#[cfg(not(feature = "uapi_v1"))]
fn v1_benchmarks(_c: &mut Criterion) {}

#[cfg(feature = "uapi_v2")]
fn v2_benchmarks(c: &mut Criterion) {
    use gpiocdev::AbiVersion::V2;
    c.bench_function("uapi_v2 get one", |b| get_one(b, V2));
    c.bench_function("uapi_v2 get ten", |b| get_ten(b, V2));
    c.bench_function("uapi_v2 get maxlen", |b| get_maxlen(b, V2));
    c.bench_function("uapi_v2 set one", |b| set_one(b, V2));
    c.bench_function("uapi_v2 set ten", |b| set_ten(b, V2));
    c.bench_function("uapi_v2 set maxlen", |b| set_maxlen(b, V2));
}
#[cfg(not(feature = "uapi_v2"))]
fn v1_benchmarks(_c: &mut Criterion) {}

// determine time taken to get one line
#[allow(unused_variables)]
fn get_one(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(10);
    let offset = 1;

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);
    let req = builder.with_line(offset).as_input().request().unwrap();

    b.iter(|| {
        let value = req.value(offset);
    });
}

// determine time taken to get ten lines
#[allow(unused_variables)]
fn get_ten(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(10);
    let offsets: Vec<Offset> = (0..10).collect();

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);
    let req = builder.with_lines(&offsets).as_input().request().unwrap();
    let mut values = Values::from_offsets(&offsets);

    b.iter(|| {
        req.values(&mut values).unwrap();
    });
}

// determine time taken to get ten lines
#[allow(unused_variables)]
fn get_maxlen(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(64);
    let offsets: Vec<Offset> = (0..64).collect();

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);
    let req = builder.with_lines(&offsets).as_input().request().unwrap();
    let mut values = Values::from_offsets(&offsets);

    b.iter(|| {
        req.values(&mut values).unwrap();
    });
}

// determine time taken to set one line
#[allow(unused_variables)]
fn set_one(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(10);
    let offset = 1;

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);
    let req = builder
        .with_line(offset)
        .as_output(Value::Active)
        .request()
        .unwrap();

    b.iter(|| {
        req.set_value(offset, Value::Active).unwrap();
    });
}

// determine time taken to set multiple lines
#[allow(unused_variables)]
fn set_ten(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(10);
    let offsets: Vec<Offset> = (0..10).collect();

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);
    let req = builder
        .with_lines(&offsets)
        .as_output(Value::Active)
        .request()
        .unwrap();
    let values = Values::from_offsets(&offsets);

    b.iter(|| {
        req.set_values(&values).unwrap();
    });
}

// determine time taken to set multiple lines
#[allow(unused_variables)]
fn set_maxlen(b: &mut Bencher, abiv: AbiVersion) {
    let s = Simpleton::new(64);
    let offsets: Vec<Offset> = (0..64).collect();

    let mut builder = Request::builder();
    builder.on_chip(s.dev_path());
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    builder.using_abi_version(abiv);
    let req = builder
        .with_lines(&offsets)
        .as_output(Value::Active)
        .request()
        .unwrap();
    let values = Values::from_offsets(&offsets);

    b.iter(|| {
        req.set_values(&values).unwrap();
    });
}
