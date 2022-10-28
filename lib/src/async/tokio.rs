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
pub struct AsyncChip(AsyncFd<Chip>);

impl AsyncChip {
    /// Create a Tokio wrapper for a Chip.
    pub fn new(chip: Chip) -> Self {
        AsyncChip(AsyncFd::new(chip).unwrap())
    }

    /// Async form of [`Chip::read_line_info_change_event`].
    pub async fn read_line_info_change_event(&self) -> Result<InfoChangeEvent> {
        loop {
            let mut guard = self.0.readable().await.map_err(crate::errno_from_ioerr)?;
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
    pub fn info_change_events(&self) -> InfoChangeStream {
        InfoChangeStream { chip: self }
    }
}

impl AsRef<Chip> for AsyncChip {
    fn as_ref(&self) -> &Chip {
        self.0.get_ref()
    }
}

/// Async form of [`InfoChangeIterator`].
///
/// [`InfoChangeIterator`]: crate::chip::InfoChangeIterator
pub struct InfoChangeStream<'a> {
    chip: &'a AsyncChip,
}

impl<'a> Stream for InfoChangeStream<'a> {
    type Item = Result<InfoChangeEvent>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let mut guard = ready!(self.chip.0.poll_read_ready(cx)).map_err(crate::errno_from_ioerr)?;
        let res = Poll::Ready(Some(self.chip.as_ref().read_line_info_change_event()));
        if !self.chip.as_ref().has_line_info_change_event()? {
            guard.clear_ready();
        }
        res
    }
}

/// Async wrapper around [`Request`] for the tokio reactor.
pub struct AsyncRequest(AsyncFd<Request>);

impl AsyncRequest {
    /// Create a Tokio wrapper for a Request.
    pub fn new(req: Request) -> Self {
        AsyncRequest(AsyncFd::new(req).unwrap())
    }

    /// Async form of [`Request::read_edge_event`].
    pub async fn read_edge_event(&self) -> Result<EdgeEvent> {
        loop {
            let mut guard = self.0.readable().await.map_err(crate::errno_from_ioerr)?;
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
    pub async fn read_edge_events_into_slice(&self, buf: &mut [u8]) -> Result<usize> {
        loop {
            let mut guard = self.0.readable().await.map_err(crate::errno_from_ioerr)?;
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
    pub fn new_edge_event_stream(&self, capacity: usize) -> EdgeEventStream {
        EdgeEventStream {
            req: self,
            events: self.0.get_ref().new_edge_event_buffer(capacity),
        }
    }

    /// Async form of [`Request::edge_events`].
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

/// Async form of [`EdgeEventBuffer`] in its role as an iterator.
pub struct EdgeEventStream<'a> {
    req: &'a AsyncRequest,
    events: EdgeEventBuffer<'a>,
}

impl<'a> Stream for EdgeEventStream<'a> {
    type Item = Result<EdgeEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        if self.events.has_event()? {
            return Poll::Ready(Some(self.events.read_event()));
        }
        let mut guard = ready!(self.req.0.poll_read_ready(cx)).map_err(crate::errno_from_ioerr)?;
        let res = Poll::Ready(Some(self.events.read_event()));
        if !self.events.has_event()? {
            guard.clear_ready();
        }
        res
    }
}
