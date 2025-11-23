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
        get_line_event, read_event, EventRequest, EventRequestFlags, LineEdgeEvent,
    };
    use gpiosim::{Level, Simpleton};
    use std::fs;

    pub fn bench(c: &mut Criterion) {
        c.bench_function("uapi_v1 edge latency", edge_latency);
        c.bench_function("uapi_v1 ten edge events", ten_edge_events);
        c.bench_function("uapi_v1 edge event object", edge_event_object);
    }

    // determine the interrupt latency.
    // overheads are toggle time.
    fn edge_latency(b: &mut Bencher) {
        let s = Simpleton::new(4);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let offset = 2;
        let er = EventRequest {
            offset,
            consumer: "edge_latency".into(),
            eventflags: EventRequestFlags::BOTH_EDGES,
            ..Default::default()
        };

        let l = get_line_event(&cf, er).expect("get_line_event should succeed");

        let mut pull = Level::High;
        let mut buf: Vec<u64> = vec![0_u64; LineEdgeEvent::u64_size()];

        b.iter(|| {
            s.set_pull(offset, pull).expect("set_pull should succeed");
            pull = match pull {
                Level::High => Level::Low,
                Level::Low => Level::High,
            };
            let _ = read_event(&l, &mut buf).expect("read_event should succeed");
        });
    }

    // determine time taken to copy ten events from the kernel buffer.
    // overheads are 10 * toggle time and 1 * latency.
    fn ten_edge_events(b: &mut Bencher) {
        let s = Simpleton::new(4);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let offset = 2;
        let er = EventRequest {
            offset,
            consumer: "ten_edge_events".into(),
            eventflags: EventRequestFlags::BOTH_EDGES,
            ..Default::default()
        };

        let l = get_line_event(&cf, er).expect("get_line_event should succeed");

        let mut pull = Level::High;
        let mut buf: Vec<u64> = vec![0_u64; LineEdgeEvent::u64_size() * 10];

        b.iter(|| {
            for _ in 0..10 {
                s.set_pull(offset, pull).expect("set_pull should succeed");
                pull = match pull {
                    Level::High => Level::Low,
                    Level::Low => Level::High,
                };
            }
            let _ = read_event(&l, &mut buf).expect("read_event should succeed");
        });
    }

    // determine the time taken to read an event from a buffer
    fn edge_event_object(b: &mut Bencher) {
        let s = Simpleton::new(4);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let offset = 2;
        let er = EventRequest {
            offset,
            consumer: "edge_event_object".into(),
            eventflags: EventRequestFlags::BOTH_EDGES,
            ..Default::default()
        };

        let l = get_line_event(&cf, er).expect("get_line_event should succeed");

        let mut buf: Vec<u64> = vec![0_u64; LineEdgeEvent::u64_size()];

        s.pullup(offset).expect("pullup should succeed");
        assert_eq!(
            read_event(&l, &mut buf).expect("read_event should succeed"),
            LineEdgeEvent::u64_size()
        );

        b.iter(|| {
            let _ = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
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
        get_line, read_event, LineConfig, LineEdgeEvent, LineFlags, LineRequest,
    };
    use gpiosim::{Level, Simpleton};
    use std::fs;

    pub fn bench(c: &mut Criterion) {
        c.bench_function("uapi_v2 edge latency", edge_latency);
        c.bench_function("uapi_v2 ten edge events", ten_edge_events);
        c.bench_function("uapi_v2 edge event object", edge_event_object);
    }

    // determine the interrupt latency.
    // overheads are toggle time.
    fn edge_latency(b: &mut Bencher) {
        let s = Simpleton::new(4);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let offset = 2;
        let mut lr = LineRequest {
            num_lines: 1,
            consumer: "edge_latency".into(),
            config: LineConfig {
                flags: LineFlags::INPUT | LineFlags::EDGE_RISING | LineFlags::EDGE_FALLING,
                ..Default::default()
            },
            ..Default::default()
        };
        lr.offsets.set(0, offset);

        let l = get_line(&cf, lr).expect("get_line should succeed");

        let mut pull = Level::High;
        let mut buf: Vec<u64> = vec![0_u64; LineEdgeEvent::u64_size()];

        b.iter(|| {
            s.set_pull(offset, pull).expect("set_pull should succeed");
            pull = match pull {
                Level::High => Level::Low,
                Level::Low => Level::High,
            };
            let _ = read_event(&l, &mut buf).expect("read_event should succeed");
        });
    }

    // determine time taken to copy ten events from the kernel buffer.
    // overheads are 10 * toggle time and 1 * latency.
    fn ten_edge_events(b: &mut Bencher) {
        let s = Simpleton::new(4);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let offset = 2;
        let mut lr = LineRequest {
            num_lines: 1,
            consumer: "ten_edge_events".into(),
            config: LineConfig {
                flags: LineFlags::INPUT | LineFlags::EDGE_RISING | LineFlags::EDGE_FALLING,
                ..Default::default()
            },
            ..Default::default()
        };
        lr.offsets.set(0, offset);

        let l = get_line(&cf, lr).expect("get_line should succeed");

        let mut pull = Level::High;
        let mut buf: Vec<u64> = vec![0_u64; LineEdgeEvent::u64_size() * 10];

        b.iter(|| {
            for _ in 0..10 {
                s.set_pull(offset, pull).expect("set_pull should succeed");
                pull = match pull {
                    Level::High => Level::Low,
                    Level::Low => Level::High,
                };
            }
            let _ = read_event(&l, &mut buf).expect("read_event should succeed");
        });
    }

    // determine the time taken to read an event from a buffer
    fn edge_event_object(b: &mut Bencher) {
        let s = Simpleton::new(4);
        let cf = fs::File::open(s.dev_path()).expect("gpiosim chip should exist");
        let offset = 2;
        let mut lr = LineRequest {
            num_lines: 1,
            consumer: "edge_event_object".into(),
            config: LineConfig {
                flags: LineFlags::INPUT | LineFlags::EDGE_RISING,
                ..Default::default()
            },
            ..Default::default()
        };
        lr.offsets.set(0, offset);

        let l = get_line(&cf, lr).expect("get_line should succeed");

        let mut buf: Vec<u64> = vec![0_u64; LineEdgeEvent::u64_size()];

        s.pullup(offset).expect("pullup should succeed");
        assert_eq!(
            read_event(&l, &mut buf).expect("read_event should succeed"),
            LineEdgeEvent::u64_size()
        );

        b.iter(|| {
            let _ = LineEdgeEvent::from_slice(&buf).expect("from_slice should succeed");
        });
    }
}
#[cfg(not(feature = "uapi_v2"))]
mod v2 {
    pub fn bench(_c: &mut criterion::Criterion) {}
}
