#[cfg(feature = "async_tokio")]
mod chip {
    use gpiocdev::chip::Chip;
    use gpiocdev::request::Request;
    use std::path::Path;

    #[cfg(feature = "uapi_v1")]
    mod uapi_v1 {
        use gpiocdev::AbiVersion::V1;

        #[tokio::test]
        async fn read_line_info_change_event() {
            super::read_line_info_change_event(V1).await
        }

        #[tokio::test]
        async fn info_change_events() {
            super::info_change_events(V1).await
        }
    }

    #[cfg(feature = "uapi_v2")]
    mod uapi_v2 {
        use gpiocdev::AbiVersion::V2;

        #[tokio::test]
        async fn read_line_info_change_event() {
            super::read_line_info_change_event(V2).await
        }

        #[tokio::test]
        async fn info_change_events() {
            super::info_change_events(V2).await
        }
    }

    async fn info_change_events(abiv: gpiocdev::AbiVersion) {
        use gpiocdev::line::InfoChangeKind;
        use gpiocdev::r#async::tokio::AsyncChip;
        use tokio_stream::StreamExt;

        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let cdevc = new_chip(&simc.dev_path, abiv);
        let chip_path = simc.dev_path.clone();
        let offset = 3;

        assert!(cdevc.watch_line_info(offset).is_ok());

        let asyncchip = AsyncChip::new(cdevc);
        let mut events = asyncchip.info_change_events();
        // request
        let req = Request::builder()
            .on_chip(chip_path)
            .with_line(offset)
            .as_input()
            .request()
            .unwrap();

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
    }

    async fn read_line_info_change_event(abiv: gpiocdev::AbiVersion) {
        use gpiocdev::r#async::tokio::AsyncChip;
        use std::time::Duration;

        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let cdevc = new_chip(&simc.dev_path, abiv);
        let chip = AsyncChip::new(cdevc);

        for offset in 0..simc.cfg.num_lines {
            assert_eq!(chip.as_ref().has_line_info_change_event(), Ok(false));
            assert!(chip.as_ref().watch_line_info(offset).is_ok());
            assert_eq!(chip.as_ref().has_line_info_change_event(), Ok(false));

            // request
            let req = Request::builder()
                .on_chip(&simc.dev_path)
                .with_line(offset)
                .as_input()
                .request()
                .unwrap();
            assert_eq!(chip.as_ref().has_line_info_change_event(), Ok(true));
            let evt = chip.read_line_info_change_event().await.unwrap();
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
            assert_eq!(chip.as_ref().has_line_info_change_event(), Ok(true));
            let evt = chip.read_line_info_change_event().await.unwrap();
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
            let evt = chip.read_line_info_change_event().await.unwrap();
            assert_eq!(evt.kind, gpiocdev::line::InfoChangeKind::Released);
            assert_eq!(evt.info.offset, offset);
            assert_eq!(evt.info.edge_detection, None);
            assert_eq!(evt.info.debounce_period, None);
        }
    }

    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn new_chip(path: &Path, abiv: gpiocdev::AbiVersion) -> gpiocdev::chip::Chip {
        let mut cdevc = Chip::from_path(path).unwrap();
        cdevc.using_abi_version(abiv);
        cdevc
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn new_chip(path: &Path, _abiv: gpiocdev::AbiVersion) -> gpiocdev::chip::Chip {
        Chip::from_path(path).unwrap()
    }
}

#[cfg(feature = "async_tokio")]
mod request {
    use gpiocdev::line::EdgeKind;
    use gpiocdev::r#async::tokio::AsyncRequest;
    use gpiocdev::request::Request;
    use tokio::time::{self, Duration};
    use tokio_stream::StreamExt;

    #[cfg(feature = "uapi_v1")]
    mod uapi_v1 {
        use gpiocdev::AbiVersion::V1;

        #[tokio::test]
        async fn read_edge_event() {
            super::read_edge_event(V1).await
        }

        #[tokio::test]
        async fn read_edge_events_into_slice() {
            super::read_edge_events_into_slice(V1).await
        }

        #[tokio::test]
        async fn new_edge_event_stream() {
            super::new_edge_event_stream(V1).await
        }

        #[tokio::test]
        async fn edge_events() {
            super::edge_events(V1).await
        }
    }

    #[cfg(feature = "uapi_v2")]
    mod uapi_v2 {
        use gpiocdev::AbiVersion::V2;

        #[tokio::test]
        async fn read_edge_event() {
            super::read_edge_event(V2).await
        }

        #[tokio::test]
        async fn read_edge_events_into_slice() {
            super::read_edge_events_into_slice(V2).await
        }

        #[tokio::test]
        async fn new_edge_event_stream() {
            super::new_edge_event_stream(V2).await
        }

        #[tokio::test]
        async fn edge_events() {
            super::edge_events(V2).await
        }
    }

    #[allow(unused)]
    async fn read_edge_event(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let offset = 2;

        let mut builder = Request::builder();
        builder
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges);

        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = AsyncRequest::new(builder.request().unwrap());

        let res = time::timeout(Duration::from_millis(10), req.read_edge_event()).await;
        assert!(res.is_err());

        simc.pullup(offset).unwrap();
        let evt = req.read_edge_event().await.unwrap();
        assert_eq!(evt.offset, offset);
        assert_eq!(evt.kind, EdgeKind::Rising);
        if abiv == gpiocdev::AbiVersion::V2 {
            assert_eq!(evt.line_seqno, 1);
        } else {
            assert_eq!(evt.line_seqno, 0);
        }

        let res = time::timeout(Duration::from_millis(10), req.read_edge_event()).await;
        assert!(res.is_err());

        simc.pulldown(offset).unwrap();
        let evt = req.read_edge_event().await.unwrap();
        assert_eq!(evt.offset, offset);
        assert_eq!(evt.kind, EdgeKind::Falling);
        if abiv == gpiocdev::AbiVersion::V2 {
            assert_eq!(evt.line_seqno, 2);
        } else {
            assert_eq!(evt.line_seqno, 0);
        }

        let res = time::timeout(Duration::from_millis(10), req.read_edge_event()).await;
        assert!(res.is_err());
    }

    async fn read_edge_events_into_slice(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(3);
        let simc = sim.chip();
        let offset = 1;

        let mut builder = Request::builder();
        builder
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges);

        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);
        let req = AsyncRequest::new(builder.request().unwrap());

        let mut buf = vec![0; req.as_ref().edge_event_size() * 3];

        // create four events
        simc.toggle(offset).unwrap();
        propagation_delay().await;
        simc.toggle(offset).unwrap();
        propagation_delay().await;
        simc.toggle(offset).unwrap();
        propagation_delay().await;
        simc.toggle(offset).unwrap();
        propagation_delay().await;

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
            .edge_event_from_slice(&buf.as_slice()[req.as_ref().edge_event_size()..])
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
            .edge_event_from_slice(&buf.as_slice()[req.as_ref().edge_event_size() * 2..])
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
        assert_eq!(wlen, req.as_ref().edge_event_size());

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
    }

    async fn new_edge_event_stream(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let offset = 2;

        let mut builder = Request::builder();
        builder
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges);

        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = AsyncRequest::new(builder.request().unwrap());

        // create four events
        simc.toggle(offset).unwrap();
        propagation_delay().await;
        simc.toggle(offset).unwrap();
        propagation_delay().await;
        simc.toggle(offset).unwrap();
        propagation_delay().await;
        simc.toggle(offset).unwrap();
        propagation_delay().await;

        let mut iter = req.new_edge_event_stream(2);

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
    }

    async fn edge_events(abiv: gpiocdev::AbiVersion) {
        let sim = gpiosim::simpleton(4);
        let simc = sim.chip();
        let offset = 0;

        let mut builder = Request::builder();
        builder
            .on_chip(&simc.dev_path)
            .with_line(offset)
            .as_input()
            .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges);

        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        builder.using_abi_version(abiv);

        let req = AsyncRequest::new(builder.request().unwrap());
        // create four events
        simc.toggle(offset).unwrap();
        propagation_delay().await;
        simc.toggle(offset).unwrap();
        propagation_delay().await;
        simc.toggle(offset).unwrap();
        propagation_delay().await;
        simc.toggle(offset).unwrap();
        propagation_delay().await;

        let mut iter = req.edge_events();

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
    }

    // allow time for a gpiosim set to propagate to cdev
    async fn propagation_delay() {
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
    }
}
