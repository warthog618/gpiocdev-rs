// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::Offset;
#[cfg(feature = "uapi_v1")]
use gpiocdev_uapi::v1;
#[cfg(feature = "uapi_v2")]
use gpiocdev_uapi::v2;
use nohash_hasher::IntMap;
#[cfg(feature = "serde")]
use serde_derive::{Deserialize, Serialize};
use std::collections::hash_map::Iter;

/// The logical level of a line.
///
/// The mapping between logical and physical levels depends on the
///  active-low setting as follows:
///
/// |             | Physical Low | Physical High |
/// |-------------|--------------|---------------|
/// | **Active-High** | Inactive | Active |
/// | **Active-Low**  | Active | Inactive |
///
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Value {
    /// The line is inactive.
    #[default]
    Inactive,
    /// The line is active.
    Active,
}
impl Value {
    /// The value opposite the current value.
    pub fn not(&self) -> Value {
        match self {
            Value::Active => Value::Inactive,
            Value::Inactive => Value::Active,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Value::Active => "active",
            Value::Inactive => "inactive",
        };
        write!(f, "{}", s)
    }
}

impl From<Value> for bool {
    fn from(l: Value) -> bool {
        match l {
            Value::Inactive => false,
            Value::Active => true,
        }
    }
}
impl From<Value> for u8 {
    fn from(l: Value) -> u8 {
        match l {
            Value::Inactive => 0,
            Value::Active => 1,
        }
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Value {
        match b {
            false => Value::Inactive,
            true => Value::Active,
        }
    }
}
impl From<u8> for Value {
    fn from(i: u8) -> Value {
        match i {
            0 => Value::Inactive,
            _ => Value::Active,
        }
    }
}

/// A  collection of line values.
///
/// Lines are identified by their offset.
#[derive(Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Values(IntMap<Offset, Value>);
impl Values {
    /// overlays the values from src over the values in the dst.
    #[cfg(feature = "uapi_v1")]
    pub(crate) fn overlay_from_v1(&mut self, offsets: &[Offset], src: &v1::LineValues) {
        for (idx, offset) in offsets.iter().enumerate() {
            self.0.insert(*offset, src.get(idx).into());
        }
    }
    /// overlays the values from src over the values in the dst.
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    pub(crate) fn overlay_from_v2(&mut self, offsets: &[Offset], src: &v2::LineValues) {
        for (idx, offset) in offsets.iter().enumerate() {
            if let Some(val) = src.get(idx) {
                self.0.insert(*offset, val.into());
            }
        }
    }

    // v1 values are a contiguous list.  If a list shorter than offsets
    // is presented to the kernel then the missing lines default to zero.
    // Build the complete values list here with any missing values being zero filled.
    #[cfg(feature = "uapi_v1")]
    pub(crate) fn to_v1(&self, offsets: &[Offset]) -> v1::LineValues {
        let mut dst: v1::LineValues = Default::default();
        for (idx, offset) in offsets.iter().enumerate() {
            if let Some(val) = self.0.get(offset) {
                dst.set(idx, (*val).into());
            }
        }
        dst
    }
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    pub(crate) fn to_v2(&self, offsets: &[Offset]) -> v2::LineValues {
        let mut dst: v2::LineValues = Default::default();
        for (idx, offset) in offsets.iter().enumerate() {
            if self.is_empty() {
                dst.set(idx, false);
            } else if let Some(val) = self.0.get(offset) {
                dst.set(idx, (*val).into());
            }
        }
        dst
    }

    /// Get the value of a line.
    #[inline]
    pub fn get(&self, offset: Offset) -> Option<Value> {
        self.0.get(&offset).copied()
    }

    /// Set the value of a line.
    #[inline]
    pub fn set(&mut self, offset: Offset, val: Value) -> &mut Self {
        self.0.insert(offset, val);
        self
    }

    /// Toggle the value of a line.
    ///
    /// If not already set then sets the line active.
    #[inline]
    pub fn toggle(&mut self, offset: Offset) {
        let val = self.0.get(&offset).copied().unwrap_or(Value::Inactive);
        self.0.insert(offset, val.not());
    }

    /// Remove any value setting for a line.
    #[inline]
    pub fn unset(&mut self, offset: Offset) {
        self.0.remove(&offset);
    }

    /// The number of lines for which values are contained in this set.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return true if the values is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Construct a `Values` collection for a set of line offsets.
    ///
    /// Typically used to create a values template to be populated via a call to
    /// [`crate::request::Request::values()`].
    ///
    /// All lines are set to inactive.
    pub fn from_offsets(offsets: &[Offset]) -> Values {
        let mut values = Values::default();
        for offset in offsets {
            values.set(*offset, Value::Inactive);
        }
        values
    }

    /// An iterator to visit all values.
    pub fn iter(&self) -> Iter<'_, Offset, Value> {
        self.0.iter()
    }

    /// Return true if values contains a matching key.
    #[cfg(feature = "uapi_v1")]
    pub(crate) fn contains_key(&self, offset: &Offset) -> bool {
        self.0.contains_key(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default() {
        let v = Value::default();
        assert_eq!(v, Value::Inactive);
    }

    #[test]
    fn not() {
        assert_eq!(Value::Active.not(), Value::Inactive);
        assert_eq!(Value::Inactive.not(), Value::Active);
    }

    #[test]
    fn from_bool() {
        assert_eq!(Value::from(true), Value::Active);
        assert_eq!(Value::from(false), Value::Inactive);
    }

    #[test]
    fn into_bool() {
        let b: bool = Value::Active.into();
        assert!(b);
        let b: bool = Value::Inactive.into();
        assert!(!b);
    }

    #[test]
    fn from_u8() {
        assert_eq!(Value::from(1), Value::Active);
        assert_eq!(Value::from(2), Value::Active);
        assert_eq!(Value::from(0), Value::Inactive);
    }

    #[test]
    fn into_u8() {
        let u: u8 = Value::Active.into();
        assert_eq!(u, 1);
        let u: u8 = Value::Inactive.into();
        assert_eq!(u, 0);
    }

    mod values {
        use super::*;

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn from_v1() {
            let offsets = Vec::from([1, 5, 3, 8]);
            let src = v1::LineValues::from_slice(&[1, 1, 0, 1]);
            let mut dst = Values::default();
            dst.set(4, Value::Active);
            dst.set(7, Value::Inactive);
            dst.overlay_from_v1(&offsets, &src);
            assert_eq!(dst.get(1), Some(Value::Active));
            assert_eq!(dst.get(2), None);
            assert_eq!(dst.get(3), Some(Value::Inactive));
            assert_eq!(dst.get(4), Some(Value::Active));
            assert_eq!(dst.get(5), Some(Value::Active));
            assert_eq!(dst.get(6), None);
            assert_eq!(dst.get(7), Some(Value::Inactive));
            assert_eq!(dst.get(8), Some(Value::Active));
        }

        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn from_v2() {
            let offsets = Vec::from([1, 5, 3, 8]);
            let mut src = v2::LineValues::default();
            src.set(0, true);
            src.set(1, true);
            src.set(2, false);
            src.set(3, true);
            let mut dst = Values::default();
            dst.set(4, Value::Active);
            dst.set(7, Value::Inactive);
            dst.overlay_from_v2(&offsets, &src);
            assert_eq!(dst.get(1), Some(Value::Active));
            assert_eq!(dst.get(2), None);
            assert_eq!(dst.get(3), Some(Value::Inactive));
            assert_eq!(dst.get(4), Some(Value::Active));
            assert_eq!(dst.get(5), Some(Value::Active));
            assert_eq!(dst.get(6), None);
            assert_eq!(dst.get(7), Some(Value::Inactive));
            assert_eq!(dst.get(8), Some(Value::Active));
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn to_v1() {
            let offsets = Vec::from([1, 5, 3, 8]);
            let mut src = Values::default();
            let dst = src.to_v1(&[]);
            assert_eq!(dst.get(0), 0); // 1
            assert_eq!(dst.get(1), 0); // 5
            assert_eq!(dst.get(2), 0); // 3
            assert_eq!(dst.get(3), 0); // 8
            src.set(1, Value::Active);
            src.set(3, Value::Inactive);
            src.set(7, Value::Active); // should be ignored
            src.set(8, Value::Active);
            let dst = src.to_v1(&offsets);
            assert_eq!(dst.get(0), 1); // 1
            assert_eq!(dst.get(1), 0); // 5
            assert_eq!(dst.get(2), 0); // 3
            assert_eq!(dst.get(3), 1); // 8
        }

        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn to_v2() {
            let offsets = Vec::from([1, 5, 3, 8]);
            let mut src = Values::default();
            let dst = src.to_v2(&[]);
            assert!(dst.bits.is_empty());
            src.set(1, Value::Active);
            src.set(3, Value::Inactive);
            src.set(7, Value::Active); // should be ignored
            src.set(8, Value::Active);
            let dst = src.to_v2(&offsets);
            assert!(dst.get(0).unwrap()); // 1
            assert!(dst.get(1).is_none()); // 5
            assert!(!dst.get(2).unwrap()); // 3
            assert!(dst.get(3).unwrap()); // 8
            assert_eq!(dst.mask.into_value(), 0b1101); // only 3 entries set
        }

        #[test]
        fn from_offsets() {
            let values = Values::from_offsets(&[1, 6, 3, 9]);
            assert_eq!(values.get(0), None);
            assert_eq!(values.get(1), Some(Value::Inactive));
            assert_eq!(values.get(2), None);
            assert_eq!(values.get(3), Some(Value::Inactive));
            assert_eq!(values.get(4), None);
            assert_eq!(values.get(5), None);
            assert_eq!(values.get(6), Some(Value::Inactive));
            assert_eq!(values.get(7), None);
            assert_eq!(values.get(8), None);
            assert_eq!(values.get(9), Some(Value::Inactive));
        }

        #[test]
        fn get() {
            let mut vv = Values::from_offsets(&[1, 2, 3]);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));

            vv.set(2, Value::Active);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Active));
            assert_eq!(vv.get(3), Some(Value::Inactive));
        }

        #[test]
        fn set() {
            let mut vv = Values::from_offsets(&[1, 2, 3]);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));

            vv.set(2, Value::Active);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Active));
            assert_eq!(vv.get(3), Some(Value::Inactive));

            vv.set(2, Value::Inactive);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));
        }

        #[test]
        fn unset() {
            let mut vv = Values::from_offsets(&[1, 2, 3]);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));
            vv.unset(2);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), None);
            assert_eq!(vv.get(3), Some(Value::Inactive));
        }

        #[test]
        fn toggle() {
            let mut vv = Values::from_offsets(&[1, 2, 3]);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));
            vv.toggle(1);
            assert_eq!(vv.get(1), Some(Value::Active));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));
            vv.toggle(2);
            assert_eq!(vv.get(1), Some(Value::Active));
            assert_eq!(vv.get(2), Some(Value::Active));
            assert_eq!(vv.get(3), Some(Value::Inactive));
            vv.toggle(1);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Active));
            assert_eq!(vv.get(3), Some(Value::Inactive));
            vv.toggle(3);
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Active));
            assert_eq!(vv.get(3), Some(Value::Active));
        }

        #[test]
        fn len() {
            assert_eq!(Values::default().len(), 0);
            assert_eq!(Values::from_offsets(&[1, 2, 3]).len(), 3);
        }

        #[test]
        fn is_empty() {
            assert!(Values::default().is_empty());
            assert!(!Values::from_offsets(&[1, 2, 3]).is_empty());
        }

        #[test]
        fn iter() {
            let mut vv = Values::from_offsets(&[1, 2, 3]);
            vv.set(2, Value::Active);
            let mut i = vv.iter();
            // assumes keys returned in order...
            assert_eq!(i.next(), Some((&1, &Value::Inactive)));
            assert_eq!(i.next(), Some((&2, &Value::Active)));
            assert_eq!(i.next(), Some((&3, &Value::Inactive)));
            assert_eq!(i.next(), None);
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn contains_key() {
            let vv = Values::from_offsets(&[1, 2, 3]);
            assert!(!vv.contains_key(&0));
            assert!(vv.contains_key(&1));
            assert!(vv.contains_key(&2));
            assert!(vv.contains_key(&3));
            assert!(!vv.contains_key(&4));
        }
    }
}
