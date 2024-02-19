// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(feature = "async_io")]
mod common;

#[cfg(feature = "async_io")]
macro_rules! common_tests {
    ($abiv:expr, $($name:ident),*) => {
        $(
            #[test]
            fn $name() {
                super::$name($abiv)
            }
        )*
        }
}

#[cfg(feature = "async_io")]
mod chip {
    use gpiocdev::{Chip, Request};
    use std::path::Path;

    #[cfg(feature = "uapi_v1")]
    mod uapi_v1 {
        common_tests! {
            gpiocdev::AbiVersion::V1,
            read_line_info_change_event,
            info_change_events
        }
    }

    #[cfg(feature = "uapi_v2")]
    mod uapi_v2 {
        common_tests! {
            gpiocdev::AbiVersion::V2,
            from_chip,
            read_line_info_change_event,
            info_change_events
        }
    }

    fn from_chip(abiv: gpiocdev::AbiVersion) {
        let s = gpiosim::Simpleton::new(4);
        let c = new_chip(s.dev_path(), abiv);
        let ac = gpiocdev::async_io::AsyncChip::from(c);
        assert_eq!(ac.as_ref().path(), s.dev_path());
        let c = Chip::from(ac);
        assert_eq!(c.path(), s.dev_path());
    }

    fn info_change_events(abiv: gpiocdev::AbiVersion) {
        use futures::stream::StreamExt;
        use gpiocdev::async_io::AsyncChip;
        use gpiocdev::line::InfoChangeKind;

        let s = gpiosim::Simpleton::new(4);
        let c = new_chip(s.dev_path(), abiv);
        let offset = 3;

        assert!(c.watch_line_info(offset).is_ok());

        let ac = AsyncChip::new(c);
        let mut events = ac.info_change_events();
        // request
        let req = Request::builder()
            .on_chip(s.dev_path())
            .with_line(offset)
            .as_input()
            .request()
            .unwrap();

        async_io::block_on(async {
            let evt = events.next().await.unwrap().unwrap();
            assert_eq!(evt.kind, InfoChangeKind::Requested);
            assert_eq!(evt.info.offset, offset);

            // reconfigure
            let mut cfg = req.config();
            cfg.with_bias(gpiocdev::line::Bias::PullUp);
            req.reconfigure(&cfg).unwrap();

            let evt = events.next().await.unwrap().unwrap();
            assert_eq!(evt.kind, InfoChangeKind::Reconfigured);
            assert_eq!(evt.info.offset, offset);

            // release
            drop(req);

            let evt = events.next().await.unwrap().unwrap();
            assert_eq!(evt.kind, InfoChangeKind::Released);
            assert_eq!(evt.info.offset, offset);
        })
    }

    fn read_line_info_change_event(abiv: gpiocdev::AbiVersion) {
        use gpiocdev::async_io::AsyncChip;
        use std::time::Duration;

        let s = gpiosim::Simpleton::new(4);
        let c = new_chip(s.dev_path(), abiv);
        let ac = AsyncChip::new(c);

        for offset in 0..s.config().num_lines {
            assert_eq!(ac.as_ref().has_line_info_change_event(), Ok(false));
            assert!(ac.as_ref().watch_line_info(offset).is_ok());
            assert_eq!(ac.as_ref().has_line_info_change_event(), Ok(false));

            // request
            let req = Request::builder()
                .on_chip(s.dev_path())
                .with_line(offset)
                .as_input()
                .request()
                .unwrap();

            async_io::block_on(async {
                assert_eq!(ac.as_ref().has_line_info_change_event(), Ok(true));
                let evt = ac.read_line_info_change_event().await.unwrap();
                assert_eq!(evt.kind, gpiocdev::line::InfoChangeKind::Requested);
                assert_eq!(evt.info.offset, offset);
                assert_eq!(evt.info.direction, gpiocdev::line::Direction::Input);
                assert_eq!(evt.info.edge_detection, None);
                assert_eq!(evt.info.edge_detection, None);
                assert_eq!(evt.info.debounce_period, None);

                // reconfigure
                let mut cfg = req.config();
                cfg.with_bias(gpiocdev::line::Bias::PullUp);
                if abiv == gpiocdev::AbiVersion::V2 {
                    cfg.with_edge_detection(gpiocdev::line::EdgeDetection::RisingEdge)
                        .with_debounce_period(Duration::from_millis(10));
                }
                req.reconfigure(&cfg).unwrap();
                assert_eq!(ac.as_ref().has_line_info_change_event(), Ok(true));
                let evt = ac.read_line_info_change_event().await.unwrap();
                assert_eq!(evt.kind, gpiocdev::line::InfoChangeKind::Reconfigured);
                assert_eq!(evt.info.offset, offset);
                assert_eq!(evt.info.direction, gpiocdev::line::Direction::Input);
                assert_eq!(evt.info.bias, Some(gpiocdev::line::Bias::PullUp));
                if abiv == gpiocdev::AbiVersion::V2 {
                    assert_eq!(
                        evt.info.edge_detection,
                        Some(gpiocdev::line::EdgeDetection::RisingEdge)
                    );
                    assert_eq!(evt.info.debounce_period, Some(Duration::from_millis(10)));
                } else {
                    assert_eq!(evt.info.edge_detection, None);
                    assert_eq!(evt.info.debounce_period, None);
                }

                // release
                drop(req);
                let evt = ac.read_line_info_change_event().await.unwrap();
                assert_eq!(evt.kind, gpiocdev::line::InfoChangeKind::Released);
                assert_eq!(evt.info.offset, offset);
                assert_eq!(evt.info.edge_detection, None);
                assert_eq!(evt.info.debounce_period, None);
            })
        }
    }

    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn new_chip(path: &Path, abiv: gpiocdev::AbiVersion) -> gpiocdev::chip::Chip {
        let mut c = Chip::from_path(path).unwrap();
        c.using_abi_version(abiv);
        c
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn new_chip(path: &Path, _abiv: gpiocdev::AbiVersion) -> gpiocdev::chip::Chip {
        Chip::from_path(path).unwrap()
    }
}

#[cfg(feature = "async_io")]
mod request {
    use crate::common::wait_propagation_delay;
    use async_std::future;
    use futures::StreamExt;
    use gpiocdev::async_io::AsyncRequest;
    use gpiocdev::line::{EdgeKind, Offset};
    use gpiocdev::Request;
    use std::path::Path;
    use std::time::Duration;

    #[cfg(feature = "uapi_v1")]
    mod uapi_v1 {
        common_tests! {
            gpiocdev::AbiVersion::V1,
            from_request,
            read_edge_event,
            read_edge_events_into_slice,
            new_edge_event_stream,
            edge_events
        }
    }

    #[cfg(feature = "uapi_v2")]
    mod uapi_v2 {
        common_tests! {
            gpiocdev::AbiVersion::V2,            read_edge_event,
            from_request,
            read_edge_events_into_slice,
            new_edge_event_stream,
            edge_events
        }
    }

    fn from_request(abiv: gpiocdev::AbiVersion) {
        use std::os::fd::AsRawFd as _;
        let s = gpiosim::Simpleton::new(4);
        let offset = 2;

        let req = new_request(s.dev_path(), offset, abiv);
        let fd = req.as_raw_fd();
        let req = AsyncRequest::from(req);
        assert_eq!(req.as_ref().as_raw_fd(), fd);
        let req = Request::from(req);
        assert_eq!(req.as_ref().as_raw_fd(), fd);
    }

    fn read_edge_event(abiv: gpiocdev::AbiVersion) {
        let s = gpiosim::Simpleton::new(4);
        let offset = 2;

        let req = AsyncRequest::new(new_request(s.dev_path(), offset, abiv));

        async_io::block_on(async {
            let res = future::timeout(Duration::from_millis(10), req.read_edge_event()).await;
            assert!(res.is_err());

            s.pullup(offset).unwrap();
            let evt = req.read_edge_event().await.unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, EdgeKind::Rising);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 1);
            } else {
                assert_eq!(evt.line_seqno, 0);
            }

            let res = future::timeout(Duration::from_millis(10), req.read_edge_event()).await;
            assert!(res.is_err());

            s.pulldown(offset).unwrap();
            let evt = req.read_edge_event().await.unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, EdgeKind::Falling);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 2);
            } else {
                assert_eq!(evt.line_seqno, 0);
            }

            let res = future::timeout(Duration::from_millis(10), req.read_edge_event()).await;
            assert!(res.is_err());
        })
    }

    fn read_edge_events_into_slice(abiv: gpiocdev::AbiVersion) {
        let s = gpiosim::Simpleton::new(3);
        let offset = 1;

        let req = AsyncRequest::new(new_request(s.dev_path(), offset, abiv));

        let evt_u64_size = req.as_ref().edge_event_u64_size();
        let mut buf = vec![0_u64; evt_u64_size * 3];

        // create four events
        s.toggle(offset).unwrap();
        wait_propagation_delay();
        s.toggle(offset).unwrap();
        wait_propagation_delay();
        s.toggle(offset).unwrap();
        wait_propagation_delay();
        s.toggle(offset).unwrap();
        wait_propagation_delay();

        async_io::block_on(async {
            // read a buffer full
            let wlen = req
                .read_edge_events_into_slice(buf.as_mut_slice())
                .await
                .unwrap();
            assert_eq!(wlen, buf.capacity());

            let evt = req.as_ref().edge_event_from_slice(buf.as_slice()).unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 1);
                assert_eq!(evt.seqno, 1);
            } else {
                assert_eq!(evt.line_seqno, 0);
                assert_eq!(evt.seqno, 0);
            }

            let evt = req
                .as_ref()
                .edge_event_from_slice(&buf.as_slice()[evt_u64_size..])
                .unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 2);
                assert_eq!(evt.seqno, 2);
            } else {
                assert_eq!(evt.line_seqno, 0);
                assert_eq!(evt.seqno, 0);
            }

            let evt = req
                .as_ref()
                .edge_event_from_slice(&buf.as_slice()[evt_u64_size * 2..])
                .unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 3);
                assert_eq!(evt.seqno, 3);
            } else {
                assert_eq!(evt.line_seqno, 0);
                assert_eq!(evt.seqno, 0);
            }

            // read remaining event
            let wlen = req
                .read_edge_events_into_slice(buf.as_mut_slice())
                .await
                .unwrap();
            assert_eq!(wlen, req.as_ref().edge_event_u64_size());

            let evt = req.as_ref().edge_event_from_slice(buf.as_slice()).unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 4);
                assert_eq!(evt.seqno, 4);
            } else {
                assert_eq!(evt.line_seqno, 0);
                assert_eq!(evt.seqno, 0);
            }
        })
    }

    fn new_edge_event_stream(abiv: gpiocdev::AbiVersion) {
        let s = gpiosim::Simpleton::new(4);
        let offset = 2;

        let req = AsyncRequest::new(new_request(s.dev_path(), offset, abiv));

        // create four events
        s.toggle(offset).unwrap();
        wait_propagation_delay();
        s.toggle(offset).unwrap();
        wait_propagation_delay();
        s.toggle(offset).unwrap();
        wait_propagation_delay();
        s.toggle(offset).unwrap();
        wait_propagation_delay();

        let mut iter = req.new_edge_event_stream(2);

        async_io::block_on(async {
            let evt = iter.next().await.unwrap().unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 1);
                assert_eq!(evt.seqno, 1);
            } else {
                assert_eq!(evt.line_seqno, 0);
                assert_eq!(evt.seqno, 0);
            }

            let evt = iter.next().await.unwrap().unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 2);
                assert_eq!(evt.seqno, 2);
            } else {
                assert_eq!(evt.line_seqno, 0);
                assert_eq!(evt.seqno, 0);
            }

            let evt = iter.next().await.unwrap().unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 3);
                assert_eq!(evt.seqno, 3);
            } else {
                assert_eq!(evt.line_seqno, 0);
                assert_eq!(evt.seqno, 0);
            }

            let evt = iter.next().await.unwrap().unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 4);
                assert_eq!(evt.seqno, 4);
            } else {
                assert_eq!(evt.line_seqno, 0);
                assert_eq!(evt.seqno, 0);
            }
        })
    }

    fn edge_events(abiv: gpiocdev::AbiVersion) {
        let s = gpiosim::Simpleton::new(4);
        let offset = 0;

        let req = AsyncRequest::new(new_request(s.dev_path(), offset, abiv));

        // create four events
        s.toggle(offset).unwrap();
        wait_propagation_delay();
        s.toggle(offset).unwrap();
        wait_propagation_delay();
        s.toggle(offset).unwrap();
        wait_propagation_delay();
        s.toggle(offset).unwrap();
        wait_propagation_delay();

        let mut iter = req.edge_events();

        async_io::block_on(async {
            let evt = iter.next().await.unwrap().unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 1);
                assert_eq!(evt.seqno, 1);
            } else {
                assert_eq!(evt.line_seqno, 0);
                assert_eq!(evt.seqno, 0);
            }

            let evt = iter.next().await.unwrap().unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 2);
                assert_eq!(evt.seqno, 2);
            } else {
                assert_eq!(evt.line_seqno, 0);
                assert_eq!(evt.seqno, 0);
            }

            let evt = iter.next().await.unwrap().unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Rising);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 3);
                assert_eq!(evt.seqno, 3);
            } else {
                assert_eq!(evt.line_seqno, 0);
                assert_eq!(evt.seqno, 0);
            }

            let evt = iter.next().await.unwrap().unwrap();
            assert_eq!(evt.offset, offset);
            assert_eq!(evt.kind, gpiocdev::line::EdgeKind::Falling);
            if abiv == gpiocdev::AbiVersion::V2 {
                assert_eq!(evt.line_seqno, 4);
                assert_eq!(evt.seqno, 4);
            } else {
                assert_eq!(evt.line_seqno, 0);
                assert_eq!(evt.seqno, 0);
            }
        })
    }

    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn new_request(path: &Path, offset: Offset, abiv: gpiocdev::AbiVersion) -> gpiocdev::Request {
        let mut builder = Request::builder();
        builder
            .on_chip(path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges);

        builder.using_abi_version(abiv);

        builder.request().unwrap()
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn new_request(path: &Path, offset: Offset, _abiv: gpiocdev::AbiVersion) -> gpiocdev::Request {
        let mut builder = Request::builder();
        builder
            .on_chip(path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
            .request()
            .unwrap()
    }
}
