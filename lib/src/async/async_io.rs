// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::chip::Chip;
use crate::line::{EdgeEvent, InfoChangeEvent};
use crate::request::{EdgeEventBuffer, Request};
use crate::Result;
use async_io::Async;
use futures::task::{Context, Poll};
use futures::{ready, Stream};
use std::pin::Pin;

/// Async wrapper around [`Chip`] for the async-io reactor.
///
/// # Example
///  ```no_run
/// # use gpiocdev::Result;
/// use gpiocdev::chip::Chip;
/// use gpiocdev::async_io::AsyncChip;
///
/// # async fn docfn() -> Result<()> {
/// let chip = Chip::from_path("/dev/gpiochip0")?;
/// let achip = AsyncChip::new(chip);
/// let evt = achip.read_line_info_change_event().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct AsyncChip(Async<Chip>);

impl AsyncChip {
    /// Create an async-io wrapper for a Chip.
    pub fn new(chip: Chip) -> Self {
        AsyncChip(Async::new(chip).expect("chip fd should be suitable as Async"))
    }

    /// Async form of [`Chip::read_line_info_change_event`].
    ///
    /// # Example
    /// ```no_run
    /// # use gpiocdev::Result;
    /// # use gpiocdev::chip::Chip;
    /// use gpiocdev::async_io::AsyncChip;
    ///
    /// # async fn docfn() -> Result<()> {
    /// let chip = Chip::from_path("/dev/gpiochip0")?;
    /// let achip = AsyncChip::new(chip);
    /// let evt = achip.read_line_info_change_event().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn read_line_info_change_event(&self) -> Result<InfoChangeEvent> {
        loop {
            self.0.readable().await?;
            let chip = self.0.get_ref();
            if chip.has_line_info_change_event()? {
                return chip.read_line_info_change_event();
            }
        }
    }

    /// Async form of [`Chip::info_change_events`].
    ///
    /// # Example
    /// ```no_run
    /// # use gpiocdev::Result;
    /// # use gpiocdev::chip::Chip;
    /// use gpiocdev::async_io::AsyncChip;
    /// use futures::StreamExt;
    ///
    /// # async fn docfn() -> Result<()> {
    /// let chip = Chip::from_path("/dev/gpiochip0")?;
    /// let achip = AsyncChip::new(chip);
    /// let mut events = achip.info_change_events();
    /// while let Ok(evt) = events.next().await.expect("got event") {
    ///     // process event...
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn info_change_events(&self) -> InfoChangeStream<'_> {
        InfoChangeStream { chip: self }
    }
}

impl AsRef<Chip> for AsyncChip {
    fn as_ref(&self) -> &Chip {
        self.0.get_ref()
    }
}

impl From<AsyncChip> for Chip {
    fn from(c: AsyncChip) -> Chip {
        c.0.into_inner()
            .expect("Chip can be unwrapped from AsyncChip")
    }
}

impl From<Chip> for AsyncChip {
    fn from(c: Chip) -> AsyncChip {
        AsyncChip::new(c)
    }
}

/// Async form of [`InfoChangeIterator`].
///
/// Created by [`AsyncChip::info_change_events`].
///
/// [`InfoChangeIterator`]: crate::chip::InfoChangeIterator
pub struct InfoChangeStream<'a> {
    chip: &'a AsyncChip,
}

impl Stream for InfoChangeStream<'_> {
    type Item = Result<InfoChangeEvent>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        ready!(self.chip.0.poll_readable(cx))?;
        Poll::Ready(Some(self.chip.as_ref().read_line_info_change_event()))
    }
}

/// Async wrapper around [`Request`] for the async-io reactor.
///
/// # Example
/// ```no_run
/// # use gpiocdev::Result;
/// use gpiocdev::Request;
/// use gpiocdev::async_io::AsyncRequest;
///
/// # async fn docfn() -> Result<()> {
/// let req = Request::builder()
///    .on_chip("/dev/gpiochip0")
///    .with_line(42)
///    .as_input()
///    .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
///    .request()?;
/// let areq = AsyncRequest::new(req);
/// let evt = areq.read_edge_event().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct AsyncRequest(Async<Request>);

impl AsyncRequest {
    /// Create an async-io wrapper for a Request.
    pub fn new(req: Request) -> Self {
        AsyncRequest(Async::new(req).expect("req fd should be suitable as Async"))
    }

    /// Async form of [`Request::read_edge_event`].
    ///
    /// # Example
    /// ```no_run
    /// # use gpiocdev::Result;
    /// use gpiocdev::Request;
    /// use gpiocdev::async_io::AsyncRequest;
    ///
    /// # async fn docfn() -> Result<()> {
    /// let req = Request::builder()
    ///    .on_chip("/dev/gpiochip0")
    ///    .with_line(42)
    ///    .as_input()
    ///    .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
    ///    .request()?;
    /// let areq = AsyncRequest::new(req);
    /// let evt = areq.read_edge_event().await?;
    /// // process event...
    /// # Ok(())
    /// # }
    /// ```
    pub async fn read_edge_event(&self) -> Result<EdgeEvent> {
        loop {
            self.0.readable().await?;
            let req = self.0.get_ref();
            if req.has_edge_event()? {
                return req.read_edge_event();
            }
        }
    }

    /// Async form of [`Request::read_edge_events_into_slice`].
    ///
    /// # Example
    /// ```no_run
    /// # use gpiocdev::Result;
    /// use gpiocdev::Request;
    /// use gpiocdev::async_io::AsyncRequest;
    ///
    /// # async fn docfn() -> Result<()> {
    /// let req = Request::builder()
    ///    .on_chip("/dev/gpiochip0")
    ///    .with_line(42)
    ///    .as_input()
    ///    .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
    ///    .request()?;
    /// let mut buf = vec![0_u64; req.edge_event_u64_size() * 3];
    /// let areq = AsyncRequest::new(req);
    /// let num_evts = areq.read_edge_events_into_slice(&mut buf).await?;
    /// // process events in buf...
    /// # Ok(())
    /// # }
    /// ```
    pub async fn read_edge_events_into_slice(&self, buf: &mut [u64]) -> Result<usize> {
        loop {
            self.0.readable().await?;
            let req = self.0.get_ref();
            if req.has_edge_event()? {
                return req.read_edge_events_into_slice(buf);
            }
        }
    }

    /// Async form of [`Request::new_edge_event_buffer`].
    ///
    /// * `capacity` - The number of events that can be buffered in user space.
    ///
    /// # Example
    /// ```no_run
    /// # use gpiocdev::Result;
    /// use gpiocdev::Request;
    /// use gpiocdev::async_io::AsyncRequest;
    /// use futures::StreamExt;
    ///
    /// # async fn docfn() -> Result<()> {
    /// let req = Request::builder()
    ///    .on_chip("/dev/gpiochip0")
    ///    .with_line(42)
    ///    .as_input()
    ///    .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
    ///    .request()?;
    /// let areq = AsyncRequest::new(req);
    /// let mut events = areq.new_edge_event_stream(2);
    /// while let Ok(evt) = events.next().await.expect("got event") {
    ///     // process event...
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn new_edge_event_stream(&self, capacity: usize) -> EdgeEventStream<'_> {
        EdgeEventStream {
            req: self,
            events: self.0.get_ref().new_edge_event_buffer(capacity),
        }
    }

    /// Async form of [`Request::edge_events`].
    ///
    /// # Example
    /// ```no_run
    /// # use gpiocdev::Result;
    /// use gpiocdev::Request;
    /// use gpiocdev::async_io::AsyncRequest;
    /// use futures::StreamExt;
    ///
    /// # async fn docfn() -> Result<()> {
    /// let req = Request::builder()
    ///    .on_chip("/dev/gpiochip0")
    ///    .with_line(42)
    ///    .as_input()
    ///    .with_edge_detection(gpiocdev::line::EdgeDetection::BothEdges)
    ///    .request()?;
    /// let areq = AsyncRequest::new(req);
    /// let mut events = areq.edge_events();
    /// while let Ok(evt) = events.next().await.expect("got event") {
    ///     // process event...
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn edge_events(&self) -> EdgeEventStream<'_> {
        EdgeEventStream {
            req: self,
            events: self.0.get_ref().edge_events(),
        }
    }
}

impl AsRef<Request> for AsyncRequest {
    fn as_ref(&self) -> &Request {
        self.0.get_ref()
    }
}

impl From<AsyncRequest> for Request {
    fn from(r: AsyncRequest) -> Request {
        r.0.into_inner()
            .expect("Request can be unwrapped from AsyncRequest")
    }
}

impl From<Request> for AsyncRequest {
    fn from(r: Request) -> AsyncRequest {
        AsyncRequest::new(r)
    }
}

/// Async form of [`EdgeEventBuffer`] in its role as an iterator.
///
/// Created by [`AsyncRequest::new_edge_event_stream`] or [`AsyncRequest::edge_events`].
pub struct EdgeEventStream<'a> {
    req: &'a AsyncRequest,
    events: EdgeEventBuffer<'a>,
}

impl Stream for EdgeEventStream<'_> {
    type Item = Result<EdgeEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        if self.events.has_event()? {
            return Poll::Ready(Some(self.events.read_event()));
        }
        ready!(self.req.0.poll_readable(cx))?;
        Poll::Ready(Some(self.events.read_event()))
    }
}
