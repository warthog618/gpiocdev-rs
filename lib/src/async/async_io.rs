// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::chip::Chip;
use crate::line::{EdgeEvent, InfoChangeEvent};
use crate::request::{EdgeEventBuffer, Request};
use crate::Result;
use async_io::Async;
use futures::task::{Context, Poll};
use futures::{ready, Stream};
use std::pin::Pin;

/// Async wrapper around [`Chip`] for the async-io reactor.
pub struct AsyncChip(Async<Chip>);

impl AsyncChip {
    /// Create an async-io wrapper for a Chip.
    pub fn new(chip: Chip) -> Self {
        AsyncChip(Async::new(chip).unwrap())
    }

    /// Async form of [`Chip::read_line_info_change_event`].
    pub async fn read_line_info_change_event(&self) -> Result<InfoChangeEvent> {
        loop {
            self.0.readable().await.map_err(crate::errno_from_ioerr)?;
            let chip = self.0.get_ref();
            if chip.has_line_info_change_event()? {
                return chip.read_line_info_change_event();
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
        ready!(self.chip.0.poll_readable(cx)).map_err(crate::errno_from_ioerr)?;
        Poll::Ready(Some(self.chip.as_ref().read_line_info_change_event()))
    }
}

/// Async wrapper around [`Request`] for the async-io reactor.
pub struct AsyncRequest(Async<Request>);

impl AsyncRequest {
    /// Create an async-io wrapper for a Request.
    pub fn new(req: Request) -> Self {
        AsyncRequest(Async::new(req).unwrap())
    }

    /// Async form of [`Request::read_edge_event`].
    pub async fn read_edge_event(&self) -> Result<EdgeEvent> {
        loop {
            self.0.readable().await.map_err(crate::errno_from_ioerr)?;
            let req = self.0.get_ref();
            if req.has_edge_event()? {
                return req.read_edge_event();
            }
        }
    }

    /// Async form of [`Request::read_edge_events_into_slice`].
    pub async fn read_edge_events_into_slice(&self, buf: &mut [u8]) -> Result<usize> {
        loop {
            self.0.readable().await.map_err(crate::errno_from_ioerr)?;
            let req = self.0.get_ref();
            if req.has_edge_event()? {
                return req.read_edge_events_into_slice(buf);
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
        ready!(self.req.0.poll_readable(cx)).map_err(crate::errno_from_ioerr)?;
        Poll::Ready(Some(self.events.read_event()))
    }
}
