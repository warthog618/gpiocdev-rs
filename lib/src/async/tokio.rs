// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::chip::Chip;
use crate::line::{EdgeEvent, InfoChangeEvent};
use crate::request::{EdgeEventBuffer, Request};
use crate::Result;
use futures::ready;
use futures::task::{Context, Poll};
use std::pin::Pin;
use tokio::io::unix::AsyncFd;
use tokio_stream::Stream;

/// Async wrapper around [`Chip`] for the tokio reactor.
///
/// # Example
///  ```no_run
/// # use gpiocdev::Result;
/// use gpiocdev::Chip;
/// use gpiocdev::tokio::AsyncChip;
///
/// # async fn docfn() -> Result<()> {
/// let chip = Chip::from_path("/dev/gpiochip0")?;
/// let achip = AsyncChip::new(chip);
/// let evt = achip.read_line_info_change_event().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct AsyncChip(AsyncFd<Chip>);

impl AsyncChip {
    /// Create a Tokio wrapper for a Chip.
    pub fn new(chip: Chip) -> Self {
        AsyncChip(AsyncFd::new(chip).unwrap())
    }

    /// Async form of [`Chip::read_line_info_change_event`].
    ///
    /// # Example
    /// ```no_run
    /// # use gpiocdev::{Chip, Result};
    /// use gpiocdev::tokio::AsyncChip;
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
            let mut guard = self.0.readable().await?;
            let chip = self.0.get_ref();
            if chip.has_line_info_change_event()? {
                let res = chip.read_line_info_change_event();
                if !chip.has_line_info_change_event()? {
                    guard.clear_ready();
                }
                return res;
            }
        }
    }

    /// Async form of [`Chip::info_change_events`].
    ///
    /// # Example
    /// ```no_run
    /// # use gpiocdev::{Chip, Result};
    /// use gpiocdev::tokio::AsyncChip;
    /// use tokio_stream::StreamExt;
    ///
    /// # async fn docfn() -> Result<()> {
    /// let chip = Chip::from_path("/dev/gpiochip0")?;
    /// let achip = AsyncChip::new(chip);
    /// let mut events = achip.info_change_events();
    /// while let Ok(evt) = events.next().await.unwrap() {
    ///     // process event...
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn info_change_events(&self) -> InfoChangeStream {
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
        let mut guard = ready!(self.chip.0.poll_read_ready(cx))?;
        let res = Poll::Ready(Some(self.chip.as_ref().read_line_info_change_event()));
        if !self.chip.as_ref().has_line_info_change_event()? {
            guard.clear_ready();
        }
        res
    }
}

/// Async wrapper around [`Request`] for the tokio reactor.
///
/// # Example
/// ```no_run
/// # use gpiocdev::Result;
/// use gpiocdev::Request;
/// use gpiocdev::tokio::AsyncRequest;
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
pub struct AsyncRequest(AsyncFd<Request>);

impl AsyncRequest {
    /// Create a Tokio wrapper for a Request.
    pub fn new(req: Request) -> Self {
        AsyncRequest(AsyncFd::new(req).unwrap())
    }

    /// Async form of [`Request::read_edge_event`].
    ///
    /// # Example
    /// ```no_run
    /// # use gpiocdev::Result;
    /// use gpiocdev::Request;
    /// use gpiocdev::tokio::AsyncRequest;
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
            let mut guard = self.0.readable().await?;
            let req = self.0.get_ref();
            if req.has_edge_event()? {
                let res = req.read_edge_event();
                if !req.has_edge_event()? {
                    guard.clear_ready();
                }
                return res;
            }
        }
    }

    /// Async form of [`Request::read_edge_events_into_slice`].
    ///
    /// # Example
    /// ```no_run
    /// # use gpiocdev::Result;
    /// use gpiocdev::Request;
    /// use gpiocdev::tokio::AsyncRequest;
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
            let mut guard = self.0.readable().await?;
            let req = self.0.get_ref();
            if req.has_edge_event()? {
                let res = req.read_edge_events_into_slice(buf);
                if !req.has_edge_event()? {
                    guard.clear_ready();
                }
                return res;
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
    /// use gpiocdev::tokio::AsyncRequest;
    /// use tokio_stream::StreamExt;
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
    /// while let Ok(evt) = events.next().await.unwrap() {
    ///     // process event...
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn new_edge_event_stream(&self, capacity: usize) -> EdgeEventStream {
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
    /// use gpiocdev::tokio::AsyncRequest;
    /// use tokio_stream::StreamExt;
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
    /// while let Ok(evt) = events.next().await.unwrap() {
    ///     // process event...
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn edge_events(&self) -> EdgeEventStream {
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
        // return any previously buffered event...
        if !self.events.is_empty() {
            return Poll::Ready(Some(self.events.read_event()));
        }
        // ... else go to the fd to check for new events
        let mut guard = ready!(self.req.0.poll_read_ready(cx))?;
        let res = Poll::Ready(Some(self.events.read_event()));
        if !self.req.0.get_ref().has_edge_event()? {
            guard.clear_ready();
        }
        res
    }
}
