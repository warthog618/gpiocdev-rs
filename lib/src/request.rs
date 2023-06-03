// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod builder;
pub use self::builder::Builder;

mod config;
pub use self::config::Config;

mod edge_event_buffer;
pub use self::edge_event_buffer::EdgeEventBuffer;

use crate::line::{self, EdgeEvent, Offset, Value, Values};
#[cfg(feature = "uapi_v1")]
use crate::AbiVersion;
use crate::{Error, Result, UapiCall};
#[cfg(not(feature = "uapi_v2"))]
use gpiocdev_uapi::v1 as uapi;
#[cfg(feature = "uapi_v1")]
use gpiocdev_uapi::v1;
#[cfg(feature = "uapi_v2")]
use gpiocdev_uapi::{v2, v2 as uapi};
use std::fs::File;
use std::mem;
use std::os::unix::prelude::{AsRawFd, RawFd};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// An active request of a set of lines.
///
/// Requests are built by the [`Builder`], which itself can be constructed by [`builder`](#method.builder).
///
/// # Output Lifetime
///
/// The value of an output line is only guaranteed for the lifetime of the request.
/// If the request is dropped then the output value becomes indeterminate - it may remain unchanged
/// or it may reset to the default value (depending on a number of factors including the kernel driver
/// for your hardware and whether other lines on the chip are still requested).
///
/// So keep the request alive to be sure of the output value.
///
/// # Event Buffering
///
/// Events are buffered both in the kernel and in user space.
/// The purpose of the kernel event buffer is to store events that may occur in a burst at
/// a faster rate than they can be handled by the user space process.
/// The kernel event buffer can be customised to some extent by the [`with_kernel_event_buffer_size`] option.
///
/// A number of options are available for user space buffering,
/// i.e. where the events are to be written when they are read from the kernel.
///
/// The [`read_edge_event`] reads a single event onto the stack.
///
/// The [`EdgeEventBuffer`] provides buffering on the heap, and can reduce reading overheads
/// by reading multiple events from the kernel at once.  The [`edge_events`] iterator uses an
/// EdgeEventBuffer, the size of which is controlled by [`with_user_event_buffer_size`]
///
/// It is also possible to read multiple events into a user specified location using
/// [`read_edge_events_into_slice`].
/// As with [`EdgeEventBuffer`] this may reduce read overheads when reading a burst of events
/// from the kernel.  The [`edge_event_size`] method provides the size required to store a
/// single event to allow sizing of custom slices.
///
/// # Reading Output Values
///
/// Note that reading back output values using [`value`] or [`values`] is dependent on driver
/// and hardware support and so cannot be guaranteed to work, though frequently it does.
/// Test with your particular hardware to be sure.
///
/// [`edge_events`]: #method.edge_events
/// [`edge_event_size`]: #method.edge_event_size
/// [`read_edge_event`]: #method.read_edge_event
/// [`read_edge_events_into_slice`]: #method.read_edge_events_into_slice
/// [`with_kernel_event_buffer_size`]: struct.Builder.html#method.with_kernel_event_buffer_size
/// [`with_user_event_buffer_size`]: struct.Builder.html#method.with_user_event_buffer_size
/// [`value`]: #method.value
/// [`values`]: #method.values
#[derive(Debug)]
pub struct Request {
    /// The request file.
    _f: File,

    /// Cached copy of _f.as_raw_fd() for syscalls, to avoid Arc<Mutex<>> overheads for ops.
    fd: RawFd,

    /// The offsets of the requested lines.
    offsets: Vec<Offset>,

    /// A snapshot of the active configuration for the request.
    cfg: Arc<RwLock<Config>>,

    /// The size of the user buffer created for the `edge_events` iterator.
    user_event_buffer_size: usize,

    /// The ABI version used to create the request, and so determines how to decode events.
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    abiv: AbiVersion,
}

impl Request {
    /// Start building a new request.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use gpiocdev::{Request, Result};
    /// # use gpiocdev::line::Values;
    /// # fn main() -> Result<()> {
    /// let req = Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_lines(&[3,5])
    ///     .request()?;
    /// let mut values = Values::default();
    /// req.values(&mut values)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Start building a new request using the provided config.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use gpiocdev::{Request, Result};
    /// # use gpiocdev::line::Value;
    /// # use gpiocdev::request::Config;
    /// # fn main() -> Result<()> {
    /// let mut cfg = Config::default();
    /// cfg.with_line(5).as_output(Value::Active);
    /// let req = Request::from_config(cfg)
    ///     .on_chip("/dev/gpiochip0")
    ///     .request()?;
    /// # req.set_value(5,Value::Inactive)?;
    /// # Ok(())
    /// # }
    pub fn from_config(config: Config) -> Builder {
        Builder::from_config(config)
    }

    /// Get the values for a subset of the requested lines.
    ///
    /// The keys indicate the lines to get.
    /// Keys that are not requested offsets are ignored.
    /// If no keys are set then all requested lines are returned.
    ///
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::{Request, Result};
    /// # use gpiocdev::line::Values;
    /// # fn main() -> Result<()> {
    /// let req = Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_lines(&[3,5,6,8])
    ///     .request()?;
    /// // subset of lines
    /// let mut values = Values::from_offsets(&[3,8]);
    /// req.values(&mut values)?;
    /// // all requested lines
    /// let mut values = Values::default();
    /// req.values(&mut values)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn values(&self, values: &mut Values) -> Result<()> {
        self.do_values(values)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_values(&self, values: &mut Values) -> Result<()> {
        match self.abiv {
            AbiVersion::V1 => self.do_values_v1(values),
            AbiVersion::V2 => self.do_values_v2(values),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_values(&self, values: &mut Values) -> Result<()> {
        self.do_values_v1(values)
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_values(&self, values: &mut Values) -> Result<()> {
        self.do_values_v2(values)
    }
    #[cfg(feature = "uapi_v1")]
    fn do_values_v1(&self, values: &mut Values) -> Result<()> {
        let mut vals = v1::LineValues::default();
        v1::get_line_values(self.fd, &mut vals)
            .map_err(|e| Error::UapiError(UapiCall::GetLineValues, e))?;
        if values.is_empty() {
            values.overlay_from_v1(&self.offsets, &vals);
            return Ok(());
        }
        for i in 0..self.offsets.len() {
            if values.contains_key(&self.offsets[i]) {
                values.set(self.offsets[i], vals.get(i).into());
            }
        }
        Ok(())
    }
    #[cfg(feature = "uapi_v2")]
    fn do_values_v2(&self, values: &mut Values) -> Result<()> {
        let mut vals = values.to_v2(&self.offsets);
        v2::get_line_values(self.fd, &mut vals)
            .map(|_| values.overlay_from_v2(&self.offsets, &vals))
            .map_err(|e| Error::UapiError(UapiCall::GetLineValues, e))
    }

    /// Get the value for one line in the request.
    ///
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::{Request, Result};
    /// # use gpiocdev::line::Values;
    /// # fn main() -> Result<()> {
    /// let req = Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_lines(&[3,5])
    ///     .request()?;
    /// let v5 = req.value(5)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn value(&self, offset: Offset) -> Result<Value> {
        if !self.offsets.contains(&offset) {
            return Err(Error::InvalidArgument(
                "offset is not a requested line.".to_string(),
            ));
        }
        let mut values = Values::default();
        values.set(offset, Value::Inactive);
        self.values(&mut values)?;
        values.get(offset).ok_or_else(|| {
            Error::UnexpectedResponse("Requested value not present in response.".to_string())
        })
    }

    /// Set the values for a subset of the requested lines.
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::{Request, Result};
    /// # use gpiocdev::line::{Value::{Active, Inactive}, Values};
    /// # fn main() -> Result<()> {
    /// let req = Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_lines(&[3,5,6,8])
    ///     .as_output(Active)
    ///     .request()?;
    /// let mut values = Values::default();
    /// values.set(5, Inactive).set(6, Inactive);
    /// req.set_values(&values)?;
    /// # Ok(())
    /// # }
    pub fn set_values(&self, values: &Values) -> Result<()> {
        self.do_set_values(values)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_set_values(&self, values: &Values) -> Result<()> {
        match self.abiv {
            AbiVersion::V1 => self.do_set_values_v1(values),
            AbiVersion::V2 => self.do_set_values_v2(values),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_set_values(&self, values: &Values) -> Result<()> {
        self.do_set_values_v1(values)
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_set_values(&self, values: &Values) -> Result<()> {
        self.do_set_values_v2(values)
    }
    #[cfg(feature = "uapi_v1")]
    fn do_set_values_v1(&self, values: &Values) -> Result<()> {
        if self.offsets.iter().any(|o| !values.contains_key(o)) {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "requires all requested lines".to_string(),
            ));
        }
        v1::set_line_values(self.fd, &values.to_v1(&self.offsets))
            .map_err(|e| Error::UapiError(UapiCall::SetLineValues, e))
    }
    #[cfg(feature = "uapi_v2")]
    fn do_set_values_v2(&self, values: &Values) -> Result<()> {
        let lv = &values.to_v2(&self.offsets);
        if lv.mask.is_empty() {
            return Err(Error::InvalidArgument(
                "no requested lines in set values.".to_string(),
            ));
        }
        v2::set_line_values(self.fd, lv).map_err(|e| Error::UapiError(UapiCall::SetLineValues, e))
    }

    /// Set the value for one line in the request.
    ///
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::{Request, Result};
    /// # use gpiocdev::line::Value;
    /// # use gpiocdev::request::Config;
    /// # fn main() -> Result<()> {
    /// let mut cfg = Config::default();
    /// cfg.with_line(5).as_output(Value::Active);
    /// let req = Request::from_config(cfg)
    ///     .on_chip("/dev/gpiochip0")
    ///     .request()?;
    /// req.set_value(5,Value::Inactive)?;
    /// # Ok(())
    /// # }
    pub fn set_value(&self, offset: Offset, value: Value) -> Result<()> {
        if !self.offsets.contains(&offset) {
            return Err(Error::InvalidArgument(
                "offset is not a requested line.".to_string(),
            ));
        }
        let mut values = Values::default();
        values.set(offset, value);
        self.set_values(&values)
    }

    /// Get a snapshot of the requested configuration.
    ///
    /// This is the configuration currently applied to the hardware.
    pub fn config(&self) -> Config {
        self.cfg
            .read()
            .expect("failed to acquire read lock on config")
            .clone()
    }

    /// Get a snapshot of the requested configuration for a particular line.
    ///
    /// This is the configuration currently applied to the line.
    pub fn line_config(&self, offset: Offset) -> Option<line::Config> {
        self.cfg
            .read()
            .expect("failed to acquire read lock on config")
            .line_config(offset)
            .cloned()
    }

    /// Reconfigure the request with the an updated configuration.
    ///
    /// Note that lines cannot be added or removed from the request.
    /// Any additional lines in `new_cfg` will be ignored, and any missing
    /// lines will retain their existing configuration.
    pub fn reconfigure(&self, new_cfg: &Config) -> Result<()> {
        let cfg = self
            .cfg
            .read()
            .expect("failed to acquire read lock on config")
            .overlay(new_cfg);
        self.do_reconfigure(&cfg)?;
        // only update request config if reconfigure succeeds.
        self.cfg
            .write()
            .expect("failed to acquire write lock on config")
            .update(cfg);
        Ok(())
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_reconfigure(&self, cfg: &Config) -> Result<()> {
        match self.abiv {
            AbiVersion::V1 => {
                if self
                    .cfg
                    .read()
                    .expect("failed to acquire read lock on config")
                    .unique()?
                    .edge_detection
                    .is_some()
                {
                    return Err(Error::AbiLimitation(
                        AbiVersion::V1,
                        "cannot reconfigure lines with edge detection".to_string(),
                    ));
                }
                if cfg.unique()?.edge_detection.is_some() {
                    return Err(Error::AbiLimitation(
                        AbiVersion::V1,
                        "cannot reconfigure edge detection".to_string(),
                    ));
                }
                v1::set_line_config(self.fd, cfg.to_v1()?)
                    .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e))
            }
            AbiVersion::V2 => v2::set_line_config(self.fd, cfg.to_v2()?)
                .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e)),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_reconfigure(&self, cfg: &Config) -> Result<()> {
        if self
            .cfg
            .read()
            .expect("failed to acquire read lock on config")
            .unique()?
            .edge_detection
            .is_some()
        {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "cannot reconfigure lines with edge detection".to_string(),
            ));
        }
        if cfg.unique()?.edge_detection.is_some() {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "cannot reconfigure edge detection".to_string(),
            ));
        }
        v1::set_line_config(self.fd, cfg.to_v1()?)
            .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e))
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_reconfigure(&self, cfg: &Config) -> Result<()> {
        v2::set_line_config(self.fd, cfg.to_v2()?)
            .map_err(|e| Error::UapiError(UapiCall::SetLineConfig, e))
    }

    /// An iterator for events from the request.
    ///
    /// By default the events are read from the kernel individually.
    ///
    /// The iterator can be backed by a user space buffer using the
    /// [`Builder.with_user_event_buffer_size`] option in the builder.
    ///
    /// # Examples
    /// ```no_run
    /// # use gpiocdev::{Request, Result};
    /// # use gpiocdev::line::{EdgeDetection, Value};
    /// # fn main() -> Result<()> {
    /// let req = Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_consumer("watcher")
    ///     .with_line(23)
    ///     .with_edge_detection(EdgeDetection::BothEdges)
    ///     .request()?;
    ///
    /// for event in req.edge_events() {
    ///     println!("{:?}", event?);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Builder.with_user_event_buffer_size`]: struct.Builder.html#method.with_user_event_buffer_size
    pub fn edge_events(&self) -> EdgeEventBuffer {
        self.new_edge_event_buffer(self.user_event_buffer_size)
    }

    /// Returns true when the request has edge events available to read using [`read_edge_event`].
    ///
    /// [`read_edge_event`]: #method.read_edge_event
    pub fn has_edge_event(&self) -> Result<bool> {
        gpiocdev_uapi::has_event(self.fd).map_err(|e| Error::UapiError(UapiCall::HasEvent, e))
    }

    /// Wait for an edge event to be available.
    ///
    /// Returns true if [`read_edge_event`] will return an event without blocking.
    ///
    /// [`read_edge_event`]: #method.read_edge_event
    pub fn wait_edge_event(&self, timeout: Duration) -> Result<bool> {
        gpiocdev_uapi::wait_event(self.fd, timeout)
            .map_err(|e| Error::UapiError(UapiCall::WaitEvent, e))
    }

    /// Read a single edge event from the request.
    ///
    /// Will block until an edge event is available.
    ///
    /// This is a convenience function.
    /// Reading events using [`edge_events`] or a buffer created using [`new_edge_event_buffer`]
    /// may be more performant.
    ///
    /// [`edge_events`]: #method.edge_events
    /// [`new_edge_event_buffer`]: #method.new_edge_event_buffer
    pub fn read_edge_event(&self) -> Result<EdgeEvent> {
        self.do_read_edge_event()
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_read_edge_event(&self) -> Result<EdgeEvent> {
        // bbuf is statically sized to the greater of the v1/v2 size so it can be placed on the stack.
        let mut bbuf = [0; mem::size_of::<v2::LineEdgeEvent>()];
        // and dynamically sliced down to the required size, if necessary
        let buf = &mut bbuf[0..self.edge_event_size()];
        let n = self.read_edge_events_into_slice(buf)?;
        self.do_edge_event_from_slice(&buf[0..n])
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_read_edge_event(&self) -> Result<EdgeEvent> {
        let mut buf = [0; mem::size_of::<uapi::LineEdgeEvent>()];
        let n = self.read_edge_events_into_slice(&mut buf)?;
        self.do_edge_event_from_slice(&buf[0..n])
    }

    /// Create an edge event buffer.
    ///
    /// * `capacity` - The number of events that can be buffered.
    pub fn new_edge_event_buffer(&self, capacity: usize) -> EdgeEventBuffer {
        EdgeEventBuffer::new(self, self.edge_event_size(), capacity)
    }

    // External buffer/slice methods.

    /// Read edge events from the kernel into a user space `[u8]` slice.
    ///
    /// This is a helper function for the special case where the user prefers to
    /// manage the buffer containing raw events, e.g. to place it in a specific
    /// location in memory.
    ///
    /// This will read in [`edge_event_size`] sized chunks so `buf` must be at least
    /// as large as one event.
    ///
    /// This function will block if no events are available to read.
    ///
    /// * `buf` - The slice to contain the raw events.
    ///
    /// [`edge_event_size`]: #method.edge_event_size
    pub fn read_edge_events_into_slice(&self, buf: &mut [u8]) -> Result<usize> {
        gpiocdev_uapi::read_event(self.fd, buf)
            .map_err(|e| Error::UapiError(UapiCall::ReadEvent, e))
    }

    /// Read an edge event from a `[u8]` slice.
    ///
    /// This is a helper function for the special case where the user prefers to
    /// manage the buffer containing raw events, e.g. to place it in a specific
    /// location in memory.
    ///
    /// Assumes the buffer has been previously populated by a call to
    /// [`read_edge_events_into_slice`].
    ///
    /// * `buf` - The slice containing the raw event.
    ///
    /// [`read_edge_events_into_slice`]: #method.read_edge_events_into_slice
    pub fn edge_event_from_slice(&self, buf: &[u8]) -> Result<EdgeEvent> {
        self.do_edge_event_from_slice(buf)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_edge_event_from_slice(&self, buf: &[u8]) -> Result<EdgeEvent> {
        Ok(match self.abiv {
            AbiVersion::V1 => {
                let mut ee = EdgeEvent::from(
                    v1::LineEdgeEvent::from_slice(buf)
                        .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
                );
                // populate offset for v1
                ee.offset = self.offsets[0];
                ee
            }
            AbiVersion::V2 => EdgeEvent::from(
                uapi::LineEdgeEvent::from_slice(buf)
                    .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
            ),
        })
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_edge_event_from_slice(&self, buf: &[u8]) -> Result<EdgeEvent> {
        let mut ee = EdgeEvent::from(
            v1::LineEdgeEvent::from_slice(buf)
                .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
        );
        // populate offset for v1
        ee.offset = self.offsets[0]; // there can be only one
        Ok(ee)
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_edge_event_from_slice(&self, buf: &[u8]) -> Result<EdgeEvent> {
        Ok(EdgeEvent::from(
            v2::LineEdgeEvent::from_slice(buf)
                .map_err(|e| Error::UapiError(UapiCall::LEEFromBuf, e))?,
        ))
    }

    /// The number of bytes required to buffer a single event read from the request.
    ///
    /// This can be used to size an external `[u8]` slice to read events into.
    /// The slice size should be a multiple of this size.
    pub fn edge_event_size(&self) -> usize {
        self.do_edge_event_size()
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_edge_event_size(&self) -> usize {
        match self.abiv {
            AbiVersion::V1 => mem::size_of::<v1::LineEdgeEvent>(),
            AbiVersion::V2 => mem::size_of::<v2::LineEdgeEvent>(),
        }
    }
    #[cfg(not(all(feature = "uapi_v1", feature = "uapi_v2")))]
    fn do_edge_event_size(&self) -> usize {
        mem::size_of::<uapi::LineEdgeEvent>()
    }
}
impl AsRawFd for Request {
    fn as_raw_fd(&self) -> i32 {
        self.fd
    }
}

#[cfg(test)]
mod tests {
    use super::Request;

    #[test]
    fn builder() {
        let b = Request::builder();
        assert_eq!(b.cfg.chip.as_os_str(), "");
        assert_eq!(b.cfg.num_lines(), 0);
        assert_eq!(b.consumer.as_str(), "");
        assert_eq!(b.kernel_event_buffer_size, 0);
        assert_eq!(b.user_event_buffer_size, 0);
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        assert_eq!(b.abiv, None);
    }

    #[test]
    fn from_config() {
        use super::line::Value;
        use super::Config;

        let mut cfg = Config::default();
        cfg.with_line(3)
            .as_input()
            .with_line(5)
            .as_output(Value::Active);
        let b = Request::from_config(cfg);
        assert_eq!(b.cfg.chip.as_os_str(), "");
        assert_eq!(b.cfg.num_lines(), 2);
        assert_eq!(b.consumer.as_str(), "");
        assert_eq!(b.kernel_event_buffer_size, 0);
        assert_eq!(b.user_event_buffer_size, 0);
        #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
        assert_eq!(b.abiv, None);
    }
}
