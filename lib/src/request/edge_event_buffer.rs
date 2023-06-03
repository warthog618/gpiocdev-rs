// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::line::EdgeEvent;
use crate::{Request, Result};
use std::cmp::max;
use std::time::Duration;

/// A user space buffer for reading edge events in bulk from a [`Request`].
///
/// Reads edge events from the kernel in bulk, where possible, while providing them
/// serially to the caller.
pub struct EdgeEventBuffer<'a> {
    req: &'a Request,

    /// The size of an individual edge event stored in the buffer.
    event_size: usize,

    /// The number of bytes currently written into the buffer
    filled: usize,

    /// The number of bytes currently read from the buffer.
    read: usize,

    /// The buffer for uAPI edge events, sized by event size and capacity
    buf: Vec<u8>,
}

impl<'a> EdgeEventBuffer<'a> {
    pub(super) fn new(req: &Request, event_size: usize, capacity: usize) -> EdgeEventBuffer {
        EdgeEventBuffer {
            req,
            event_size,
            filled: 0,
            read: 0,
            buf: vec![0; max(capacity, 1) * event_size],
        }
    }

    /// The number of events that can be stored in the buffer.
    pub fn capacity(&self) -> usize {
        self.buf.capacity() / self.event_size
    }

    /// The number of unread events currently stored in this buffer.
    ///
    /// This does not include events which may be buffered in the kernel.
    pub fn len(&self) -> usize {
        (self.filled - self.read) / self.event_size
    }

    /// Returns true if there are no unread events in the buffer.
    ///
    /// This does not check whether events are available in the kernel buffer,
    /// only if there are events currently stored in this buffer.
    pub fn is_empty(&self) -> bool {
        self.read >= self.filled
    }

    /// Returns true when either the buffer, or the request, has edge events available to read.
    pub fn has_event(&mut self) -> Result<bool> {
        if self.read < self.filled {
            return Ok(true);
        }
        self.req.has_edge_event()
    }

    /// Returns the next event from the buffer.
    ///
    /// If the buffer is empty then events are read from the request, which may block if no
    /// events are available. To avoid blocking, check [`has_event`] first, or use
    /// [`wait_event`] with a suitable timeout.
    ///
    /// [`has_event`]: #method.has_event
    /// [`wait_event`]: #method.wait_event
    pub fn read_event(&mut self) -> Result<EdgeEvent> {
        if self.read < self.filled {
            let evt_end = self.read + self.event_size;
            let evt = &self.buf[self.read..evt_end];
            self.read = evt_end;
            return self.req.edge_event_from_slice(evt);
        }
        self.read = 0;
        self.filled = 0;
        let n = self.req.read_edge_events_into_slice(&mut self.buf)?;
        // Could turn these into run-time errors, but they should never happen
        // so make them asserts to keep it simple.
        assert!(n > 0);
        assert_eq!(n % self.event_size, 0);
        self.filled = n;
        self.read = self.event_size;
        self.req
            .edge_event_from_slice(&self.buf[0..self.event_size])
    }

    /// Wait for an edge event from the request.
    ///
    /// * `timeout` - The maximum time to wait for an event.
    pub fn wait_event(&mut self, timeout: Duration) -> Result<EdgeEvent> {
        self.req.wait_edge_event(timeout)?;
        self.read_event()
    }
}

impl<'a> Iterator for EdgeEventBuffer<'a> {
    type Item = Result<EdgeEvent>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.read_event())
    }
}
