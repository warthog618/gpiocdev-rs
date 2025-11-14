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
use std::os::unix::prelude::{AsFd, AsRawFd, BorrowedFd};
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
    f: File,

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
    /// # fn example() -> Result<(), gpiocdev::Error> {
    /// # use gpiocdev::line::Values;
    /// let req = gpiocdev::Request::builder()
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
    /// # fn example() -> Result<(), gpiocdev::Error> {
    /// # use gpiocdev::line::Value;
    /// # use gpiocdev::request::Config;
    /// let mut cfg = Config::default();
    /// cfg.with_line(5).as_output(Value::Active);
    /// let req = gpiocdev::Request::from_config(cfg)
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
    /// # fn example() -> Result<(), gpiocdev::Error> {
    /// # use gpiocdev::line::Values;
    /// let req = gpiocdev::Request::builder()
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
        v1::get_line_values(&self.f, &mut vals)
            .map(|_| values.update_from_v1(&self.offsets, &vals))
            .map_err(|e| Error::Uapi(UapiCall::GetLineValues, e))
    }
    #[cfg(feature = "uapi_v2")]
    fn do_values_v2(&self, values: &mut Values) -> Result<()> {
        let mut vals = values.to_v2(&self.offsets);
        v2::get_line_values(&self.f, &mut vals)
            .map(|_| values.update_from_v2(&self.offsets, &vals))
            .map_err(|e| Error::Uapi(UapiCall::GetLineValues, e))
    }

    /// Get the value for one line in the request.
    ///
    /// # Examples
    /// ```no_run
    /// # fn example() -> Result<(), gpiocdev::Error> {
    /// # use gpiocdev::line::Values;
    /// let req = gpiocdev::Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_lines(&[3,5])
    ///     .request()?;
    /// let v5 = req.value(5)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn value(&self, offset: Offset) -> Result<Value> {
        let idx = self
            .offsets
            .iter()
            .position(|v| v == &offset)
            .ok_or_else(|| Error::InvalidArgument("offset is not a requested line.".into()))?;
        self.do_value(idx)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_value(&self, idx: usize) -> Result<Value> {
        match self.abiv {
            AbiVersion::V1 => self.do_value_v1(idx),
            AbiVersion::V2 => self.do_value_v2(idx),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_value(&self, idx: usize) -> Result<Value> {
        self.do_value_v1(idx)
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_value(&self, idx: usize) -> Result<Value> {
        self.do_value_v2(idx)
    }
    #[cfg(feature = "uapi_v1")]
    fn do_value_v1(&self, idx: usize) -> Result<Value> {
        let mut vals = v1::LineValues::default();
        v1::get_line_values(&self.f, &mut vals)
            .map_err(|e| Error::Uapi(UapiCall::GetLineValues, e))?;
        Ok(vals.get(idx).into())
    }
    #[cfg(feature = "uapi_v2")]
    fn do_value_v2(&self, idx: usize) -> Result<Value> {
        let mut vals = v2::LineValues {
            mask: 0x01 << idx,
            ..Default::default()
        };
        v2::get_line_values(&self.f, &mut vals)
            .map_err(|e| Error::Uapi(UapiCall::GetLineValues, e))?;
        Ok(vals.get(idx).unwrap().into())
    }

    /// Get the value for the lone line in the request.
    ///
    /// This is a simplified version of [`value`] for single line request,
    /// so the line offset is not required.
    ///
    /// # Examples
    /// ```no_run
    /// # fn example() -> Result<(), gpiocdev::Error> {
    /// # use gpiocdev::line::Values;
    /// let req = gpiocdev::Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_line(3)
    ///     .request()?;
    /// // get the value of line 3 - the only line in the request
    /// let v = req.lone_value()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`value`]: method.value
    pub fn lone_value(&self) -> Result<Value> {
        if self.offsets.len() != 1 {
            return Err(Error::InvalidArgument(
                "request contains multiple lines.".into(),
            ))?;
        }
        self.do_value(0)
    }

    /// Set the values for a subset of the requested lines.
    /// # Examples
    /// ```no_run
    /// # fn example() -> Result<(), gpiocdev::Error> {
    /// # use gpiocdev::line::{Value::{Active, Inactive}, Values};
    /// let req = gpiocdev::Request::builder()
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
        if !values.contains_keys(&self.offsets) {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "requires all requested lines".into(),
            ));
        }
        v1::set_line_values(&self.f, &values.to_v1(&self.offsets))
            .map_err(|e| Error::Uapi(UapiCall::SetLineValues, e))
    }
    #[cfg(feature = "uapi_v2")]
    fn do_set_values_v2(&self, values: &Values) -> Result<()> {
        let lv = &values.to_v2(&self.offsets);
        if lv.mask == 0 {
            return Err(Error::InvalidArgument(
                "no requested lines in set values.".into(),
            ));
        }
        v2::set_line_values(&self.f, lv).map_err(|e| Error::Uapi(UapiCall::SetLineValues, e))
    }

    /// Set the value for one line in the request.
    ///
    /// # Examples
    /// ```no_run
    /// # fn example() -> Result<(), gpiocdev::Error> {
    /// # use gpiocdev::line::Value;
    /// let req = gpiocdev::Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_lines(&[5,6])
    ///     .as_output(Value::Active)
    ///     .request()?;
    /// req.set_value(5,Value::Inactive)?;
    /// # Ok(())
    /// # }
    pub fn set_value(&self, offset: Offset, value: Value) -> Result<()> {
        let idx = self
            .offsets
            .iter()
            .position(|v| v == &offset)
            .ok_or_else(|| Error::InvalidArgument("offset is not a requested line.".into()))?;
        self.do_set_value(idx, value)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_set_value(&self, idx: usize, value: Value) -> Result<()> {
        match self.abiv {
            AbiVersion::V1 => self.do_set_value_v1(idx, value),
            AbiVersion::V2 => self.do_set_value_v2(idx, value),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_set_value(&self, idx: usize, value: Value) -> Result<()> {
        self.do_set_value_v1(idx, value)
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_set_value(&self, idx: usize, value: Value) -> Result<()> {
        self.do_set_value_v2(idx, value)
    }
    #[cfg(feature = "uapi_v1")]
    fn do_set_value_v1(&self, idx: usize, value: Value) -> Result<()> {
        if self.offsets.len() > 1 {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "requires all requested lines".into(),
            ));
        }
        let mut vals = v1::LineValues::default();
        vals.set(idx, value.into());
        v1::set_line_values(&self.f, &vals).map_err(|e| Error::Uapi(UapiCall::SetLineValues, e))
    }
    #[cfg(feature = "uapi_v2")]
    fn do_set_value_v2(&self, idx: usize, value: Value) -> Result<()> {
        let mut vals = v2::LineValues::default();
        let mask = 0x01 << idx;
        if value == Value::Active {
            vals.bits |= mask;
        }
        vals.mask |= mask;
        v2::set_line_values(&self.f, &vals).map_err(|e| Error::Uapi(UapiCall::SetLineValues, e))
    }

    /// Set the value for the lone line in the request.
    ///
    /// This is a simplified version of [`set_value`] intended for single line requests,
    /// so the line offset is not required.
    ///
    /// # Examples
    /// ```no_run
    /// # fn example() -> Result<(), gpiocdev::Error> {
    /// # use gpiocdev::line::Value;
    /// let req = gpiocdev::Request::builder()
    ///     .on_chip("/dev/gpiochip0")
    ///     .with_line(5)
    ///     .as_output(Value::Active)
    ///     .request()?;
    /// // set the value of line 5 - the only line in the request
    /// req.set_lone_value(Value::Inactive)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`set_value`]: method.set_value
    pub fn set_lone_value(&self, value: Value) -> Result<()> {
        if self.offsets.len() != 1 {
            return Err(Error::InvalidArgument(
                "request contains multiple lines.".into(),
            ))?;
        }
        self.do_set_value(0, value)
    }

    /// Return the path of the chip for this request.
    pub fn chip_path(&self) -> std::path::PathBuf {
        self.cfg
            .read()
            .expect("failed to acquire read lock on config")
            .chip
            .clone()
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
                        "cannot reconfigure lines with edge detection".into(),
                    ));
                }
                if cfg.unique()?.edge_detection.is_some() {
                    return Err(Error::AbiLimitation(
                        AbiVersion::V1,
                        "cannot reconfigure edge detection".into(),
                    ));
                }
                v1::set_line_config(&self.f, cfg.to_v1()?)
                    .map_err(|e| Error::Uapi(UapiCall::SetLineConfig, e))
            }
            AbiVersion::V2 => v2::set_line_config(&self.f, cfg.to_v2()?)
                .map_err(|e| Error::Uapi(UapiCall::SetLineConfig, e)),
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
                "cannot reconfigure lines with edge detection".into(),
            ));
        }
        if cfg.unique()?.edge_detection.is_some() {
            return Err(Error::AbiLimitation(
                AbiVersion::V1,
                "cannot reconfigure edge detection".into(),
            ));
        }
        v1::set_line_config(&self.f, cfg.to_v1()?)
            .map_err(|e| Error::Uapi(UapiCall::SetLineConfig, e))
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_reconfigure(&self, cfg: &Config) -> Result<()> {
        v2::set_line_config(&self.f, cfg.to_v2()?)
            .map_err(|e| Error::Uapi(UapiCall::SetLineConfig, e))
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
    /// # fn example() -> Result<(), gpiocdev::Error> {
    /// # use gpiocdev::Request;
    /// # use gpiocdev::line::{EdgeDetection, Value};
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
    pub fn edge_events(&self) -> EdgeEventBuffer<'_> {
        self.new_edge_event_buffer(self.user_event_buffer_size)
    }

    /// Returns true when the request has edge events available to read using [`read_edge_event`].
    ///
    /// [`read_edge_event`]: #method.read_edge_event
    pub fn has_edge_event(&self) -> Result<bool> {
        gpiocdev_uapi::has_event(&self.f).map_err(|e| Error::Uapi(UapiCall::HasEvent, e))
    }

    /// Wait for an edge event to be available.
    ///
    /// Returns true if [`read_edge_event`] will return an event without blocking.
    ///
    /// [`read_edge_event`]: #method.read_edge_event
    pub fn wait_edge_event(&self, timeout: Duration) -> Result<bool> {
        gpiocdev_uapi::wait_event(&self.f, timeout).map_err(|e| Error::Uapi(UapiCall::WaitEvent, e))
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
        let mut bbuf = [0; mem::size_of::<v2::LineEdgeEvent>() / 8];
        // and dynamically sliced down to the required size, if necessary
        let buf = &mut bbuf[0..self.edge_event_u64_size()];
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
    pub fn new_edge_event_buffer(&self, capacity: usize) -> EdgeEventBuffer<'_> {
        EdgeEventBuffer::new(self, self.edge_event_size(), capacity)
    }

    // External buffer/slice methods.

    /// Read edge events from the kernel into a user space `[u64]` slice.
    ///
    /// This is a helper function for the special case where the user prefers to
    /// manage the buffer containing raw events, e.g. to place it in a specific
    /// location in memory.
    ///
    /// The slice is `u64` to satisfy alignment requirements on 32bit platforms.
    ///
    /// This will read in [`edge_event_size`] sized chunks so `buf` must be at least
    /// as large as one event. e.g. `vec![0_u64; edge_event_u64_size()]`
    ///
    /// This function will block if no events are available to read.
    ///
    /// Returns the number of u64 words read.
    ///
    /// * `buf` - The slice to contain the raw events.
    ///
    /// [`edge_event_size`]: #method.edge_event_size
    pub fn read_edge_events_into_slice(&self, buf: &mut [u64]) -> Result<usize> {
        gpiocdev_uapi::read_event(&self.f, buf).map_err(|e| Error::Uapi(UapiCall::ReadEvent, e))
    }

    /// Read an edge event from a `[u64]` slice.
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
    pub fn edge_event_from_slice(&self, buf: &[u64]) -> Result<EdgeEvent> {
        self.do_edge_event_from_slice(buf)
    }
    #[cfg(all(feature = "uapi_v1", feature = "uapi_v2"))]
    fn do_edge_event_from_slice(&self, buf: &[u64]) -> Result<EdgeEvent> {
        match self.abiv {
            AbiVersion::V1 => {
                let mut ee = EdgeEvent::try_from(
                    v1::LineEdgeEvent::from_slice(buf)
                        .map_err(|e| Error::Uapi(UapiCall::LEEFromBuf, e))?,
                )?;
                // populate offset for v1
                ee.offset = self.offsets[0];
                Ok(ee)
            }
            AbiVersion::V2 => EdgeEvent::try_from(
                uapi::LineEdgeEvent::from_slice(buf)
                    .map_err(|e| Error::Uapi(UapiCall::LEEFromBuf, e))?,
            ),
        }
    }
    #[cfg(not(feature = "uapi_v2"))]
    fn do_edge_event_from_slice(&self, buf: &[u64]) -> Result<EdgeEvent> {
        let mut ee = EdgeEvent::try_from(
            v1::LineEdgeEvent::from_slice(buf).map_err(|e| Error::Uapi(UapiCall::LEEFromBuf, e))?,
        )?;
        // populate offset for v1
        ee.offset = self.offsets[0]; // there can be only one
        Ok(ee)
    }
    #[cfg(not(feature = "uapi_v1"))]
    fn do_edge_event_from_slice(&self, buf: &[u64]) -> Result<EdgeEvent> {
        EdgeEvent::try_from(
            v2::LineEdgeEvent::from_slice(buf).map_err(|e| Error::Uapi(UapiCall::LEEFromBuf, e))?,
        )
    }

    /// The number of u64s required to buffer a single event read from the request.
    ///
    /// This can be used to size an external `[u64]` slice to read events into.
    /// The slice size should be a multiple of this size.
    /// e.g. `vec![0_u64; edge_event_u64_size()]` is a buffer large enough for one event.
    pub fn edge_event_u64_size(&self) -> usize {
        self.do_edge_event_size() / 8
    }

    /// The number of bytes required to buffer a single event read from the request.
    ///
    /// This can be used to size an external `[u64]` slice to read events into.
    /// The slice size should be a multiple of this size.
    /// e.g. `vec![0_u64; edge_event_size()]` is a buffer large enough for 8 events.
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

impl AsFd for Request {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.f.as_fd()
    }
}

impl AsRawFd for Request {
    #[inline]
    fn as_raw_fd(&self) -> i32 {
        self.f.as_raw_fd()
    }
}

impl AsRef<Request> for Request {
    #[inline]
    fn as_ref(&self) -> &Request {
        self
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
