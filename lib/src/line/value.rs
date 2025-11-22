// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::Offset;
#[cfg(feature = "uapi_v1")]
use gpiocdev_uapi::v1;
#[cfg(feature = "uapi_v2")]
use gpiocdev_uapi::v2;
#[cfg(feature = "serde")]
use serde_derive::{Deserialize, Serialize};
use std::cmp::Ordering;

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
        write!(f, "{s}")
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

/// The value of a particular line.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LineValue {
    /// the offset of the line
    pub offset: Offset,
    /// the value of the line
    pub value: Value,
}

/// A  collection of line values.
///
/// Lines are identified by their offset.
#[derive(Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Values(Vec<LineValue>);
impl Values {
    // updates the values in dst with values from src.
    //
    // Both sets of offsets must be sorted.
    #[cfg(feature = "uapi_v1")]
    pub(crate) fn update_from_v1(&mut self, offsets: &[Offset], src: &v1::LineValues) {
        // requested full set
        if self.0.is_empty() {
            self.0.reserve_exact(offsets.len());
            for (idx, offset) in offsets.iter().enumerate() {
                self.0.push(LineValue {
                    offset: *offset,
                    value: Value::from(src.get(idx)),
                });
            }
            return;
        }
        // requested explicit set
        let mut sidx = 0;
        for lv in self.0.iter_mut() {
            // values is a superset of src, and both are sorted...
            while sidx < offsets.len() {
                match lv.offset.cmp(&offsets[sidx]) {
                    Ordering::Less => {
                        // offset is self, but not src, so skip it
                        break;
                    }
                    Ordering::Equal => {
                        lv.value = Value::from(src.get(sidx));
                        sidx += 1;
                        break;
                    }
                    Ordering::Greater => {
                        // offset is in src, but not self, so ignore it
                        sidx += 1;
                        continue;
                    }
                }
            }
        }
    }

    // updates the values with values from src.
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    pub(crate) fn update_from_v2(&mut self, offsets: &[Offset], src: &v2::LineValues) {
        // requested full set
        if self.0.is_empty() {
            self.0.reserve_exact(offsets.len());
            for (idx, offset) in offsets.iter().enumerate() {
                self.0.push(LineValue {
                    offset: *offset,
                    value: Value::from(src.get(idx).expect("idx should exist")),
                });
            }
            return;
        }
        // requested explicit set
        let mut didx = 0;
        for (idx, offset) in offsets.iter().enumerate() {
            if let Some(v) = src.get(idx) {
                // self is a superset of src, and both are sorted, so scan to find offset
                loop {
                    if let Some(lv) = self.0.get_mut(didx) {
                        match lv.offset.cmp(offset) {
                            Ordering::Less => {
                                // offset is in src, but not dst, so skip it
                                didx += 1;
                                continue;
                            }
                            Ordering::Equal => {
                                lv.value = Value::from(v);
                                didx += 1;
                                break;
                            }
                            Ordering::Greater => {
                                // offset is in values, but not offsets, so ignore it
                                break;
                            }
                        }
                    }
                    return;
                }
            }
        }
    }

    // v1 values are a contiguous list.  If a list shorter than offsets
    // is presented to the kernel then the missing lines default to zero.
    // Build the complete values list here with any missing values being zero filled.
    ///
    /// Both sets of offsets must be sorted.
    #[cfg(feature = "uapi_v1")]
    pub(crate) fn to_v1(&self, offsets: &[Offset]) -> v1::LineValues {
        let mut dst: v1::LineValues = Default::default();
        let mut didx = 0;
        for lv in self.0.iter() {
            while didx < offsets.len() {
                match lv.offset.cmp(&offsets[didx]) {
                    Ordering::Less => {
                        // offset is in values, but not offsets, so ignore it
                        break;
                    }
                    Ordering::Equal => {
                        dst.set(didx, lv.value.into());
                        didx += 1;
                        break;
                    }
                    Ordering::Greater => {
                        // offset is in offsets, but not values, so it defaults
                        didx += 1;
                        continue;
                    }
                }
            }
        }
        dst
    }

    // v2 values are a bitmap in offsets ordering
    ///
    /// Both sets of offsets must be sorted.
    #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
    pub(crate) fn to_v2(&self, offsets: &[Offset]) -> v2::LineValues {
        let mut dst: v2::LineValues = Default::default();
        if self.0.is_empty() {
            if offsets.len() < 64 {
                dst.mask = (0x01_u64 << offsets.len()) - 1;
            } else {
                dst.mask = !0_u64;
            }
            return dst;
        }
        let mut didx = 0;
        for lv in self.0.iter() {
            while didx < offsets.len() {
                match lv.offset.cmp(&offsets[didx]) {
                    Ordering::Less => {
                        // offset is in values, but not offsets, so ignore it
                        break;
                    }
                    Ordering::Equal => {
                        dst.set(didx, lv.value.into());
                        didx += 1;
                        break;
                    }
                    Ordering::Greater => {
                        // offset is in offsets, but not values, so it defaults
                        didx += 1;
                        continue;
                    }
                }
            }
        }
        dst
    }

    /// Get the value of a line.
    #[inline]
    pub fn get(&self, offset: Offset) -> Option<Value> {
        match self.0.binary_search_by(|lv| lv.offset.cmp(&offset)) {
            Ok(idx) => Some(self.0.get(idx).expect("idx should exist").value),
            Err(_idx) => None,
        }
    }

    /// Set the value of a line.
    #[inline]
    pub fn set(&mut self, offset: Offset, value: Value) -> &mut Self {
        // fast path - appending
        if let Some(last) = self.0.last() {
            if offset > last.offset {
                self.0.push(LineValue { offset, value });
                return self;
            }
        }
        // slow path - inserting
        match self.0.binary_search_by(|lv| lv.offset.cmp(&offset)) {
            Ok(idx) => {
                self.0.get_mut(idx).expect("idx should exist").value = value;
            }
            Err(idx) => self.0.insert(idx, LineValue { offset, value }),
        }
        self
    }

    /// Toggle all values.
    pub fn not(&mut self) -> &mut Self {
        for lv in self.0.iter_mut() {
            lv.value = lv.value.not();
        }
        self
    }

    /// Toggle the value of a line.
    ///
    /// If not already set then sets the line active.
    #[inline]
    pub fn toggle(&mut self, offset: Offset) {
        match self.0.binary_search_by(|lv| lv.offset.cmp(&offset)) {
            Ok(idx) => {
                let lv = self.0.get_mut(idx).expect("idx should exist");
                lv.value = lv.value.not();
            }
            Err(idx) => self.0.insert(
                idx,
                LineValue {
                    offset,
                    value: Value::Active,
                },
            ),
        }
    }

    /// Remove any value setting for a line.
    #[inline]
    pub fn unset(&mut self, offset: Offset) {
        if let Ok(idx) = self.0.binary_search_by(|lv| lv.offset.cmp(&offset)) {
            self.0.remove(idx);
        }
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
    /// [`crate::Request::values()`].
    ///
    /// All lines are set to inactive.
    pub fn from_offsets(offsets: &[Offset]) -> Values {
        offsets.iter().collect()
    }

    /// An iterator to visit all values.
    pub fn iter(&self) -> std::slice::Iter<'_, LineValue> {
        self.0.iter()
    }

    /// An mutating iterator to visit all values.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, LineValue> {
        self.0.iter_mut()
    }

    /// Returns true if values are defined for all offsets.
    ///
    /// Offsets must be sorted.
    #[cfg(feature = "uapi_v1")]
    pub(crate) fn contains_keys(&self, offsets: &[Offset]) -> bool {
        let mut start_idx = 0;
        for offset in offsets.iter() {
            match self.0[start_idx..self.0.len()]
                .iter()
                .position(|lv| &lv.offset == offset)
            {
                Some(idx) => {
                    start_idx += idx + 1;
                    continue;
                }
                None => return false,
            }
        }
        true
    }
}

impl<'a> FromIterator<&'a Offset> for Values {
    fn from_iter<I: IntoIterator<Item = &'a Offset>>(iter: I) -> Self {
        let mut values = Values::default();
        // not quite as fast as append and sort, but also performs de-duping.
        for offset in iter {
            values.set(*offset, Value::Inactive);
        }
        values
    }
}

impl FromIterator<(Offset, Value)> for Values {
    fn from_iter<I: IntoIterator<Item = (Offset, Value)>>(iter: I) -> Self {
        let mut values = Values::default();
        // not quite as fast as append and sort, but also performs de-duping.
        for (offset, value) in iter {
            values.set(offset, value);
        }
        values
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
            // offset must be sorted
            let offsets = Vec::from([1, 3, 5, 8]);
            let src = v1::LineValues::from_slice(&[1, 0, 1, 1]);
            // full set
            let mut dst = Values::default();
            dst.update_from_v1(&offsets, &src);
            assert_eq!(dst.get(1), Some(Value::Active));
            assert_eq!(dst.get(2), None);
            assert_eq!(dst.get(3), Some(Value::Inactive));
            assert_eq!(dst.get(4), None);
            assert_eq!(dst.get(5), Some(Value::Active));
            assert_eq!(dst.get(6), None);
            assert_eq!(dst.get(7), None);
            assert_eq!(dst.get(8), Some(Value::Active));

            // explicit set
            let mut dst = Values::default();
            dst.set(1, Value::Inactive);
            dst.set(3, Value::Active);
            dst.set(4, Value::Active);
            dst.set(7, Value::Inactive);
            dst.update_from_v1(&offsets, &src);
            assert_eq!(dst.get(1), Some(Value::Active));
            assert_eq!(dst.get(2), None);
            assert_eq!(dst.get(3), Some(Value::Inactive));
            assert_eq!(dst.get(4), Some(Value::Active));
            assert_eq!(dst.get(5), None);
            assert_eq!(dst.get(6), None);
            assert_eq!(dst.get(7), Some(Value::Inactive));
            assert_eq!(dst.get(8), None);
        }

        #[test]
        #[cfg(any(feature = "uapi_v2", not(feature = "uapi_v1")))]
        fn from_v2() {
            // offsets must be sorted
            let offsets = Vec::from([1, 3, 5, 8]);
            let mut src = v2::LineValues::default();
            src.set(0, true);
            src.set(1, false);
            src.set(2, true);
            src.set(3, true);
            let mut dst = Values::default();
            dst.set(3, Value::Active);
            dst.set(4, Value::Active);
            dst.set(5, Value::Inactive);
            dst.set(7, Value::Inactive);
            dst.update_from_v2(&offsets, &src);
            assert_eq!(dst.get(1), None);
            assert_eq!(dst.get(2), None);
            assert_eq!(dst.get(3), Some(Value::Inactive));
            assert_eq!(dst.get(4), Some(Value::Active));
            assert_eq!(dst.get(5), Some(Value::Active));
            assert_eq!(dst.get(6), None);
            assert_eq!(dst.get(7), Some(Value::Inactive));
            assert_eq!(dst.get(8), None);
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn to_v1() {
            // offsets must be sorted.
            let offsets = Vec::from([1, 3, 5, 8]);
            // both empty
            let mut src = Values::default();
            let dst = src.to_v1(&[]);
            assert_eq!(dst.get(0), 0); // 1
            assert_eq!(dst.get(1), 0); // 5
            assert_eq!(dst.get(2), 0); // 3
            assert_eq!(dst.get(3), 0); // 8

            let dst = src.to_v1(&offsets);
            assert_eq!(dst.get(0), 0); // 1
            assert_eq!(dst.get(1), 0); // 5
            assert_eq!(dst.get(2), 0); // 3
            assert_eq!(dst.get(3), 0); // 8

            // values intersects offsets
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
            // offsets must be sorted.
            let offsets = Vec::from([1, 3, 5, 8]);
            // both empty
            let mut src = Values::default();
            let dst = src.to_v2(&[]);
            assert_eq!(dst.bits, 0);
            assert_eq!(dst.mask, 0);
            // empty values
            let dst = src.to_v2(&offsets);
            assert_eq!(dst.bits, 0);
            assert_eq!(dst.mask, 0b01111);

            // values intersects offsets
            src.set(1, Value::Active);
            src.set(3, Value::Inactive);
            src.set(7, Value::Active); // should be ignored
            src.set(8, Value::Active);
            let dst = src.to_v2(&offsets);
            assert!(dst.get(0).expect("idx should exist")); // 1
            assert!(!dst.get(1).expect("idx should exist")); // 3
            assert!(dst.get(2).is_none()); // 5
            assert!(dst.get(3).expect("idx should exist")); // 8
            assert_eq!(dst.mask, 0b1011); // only 3 entries set
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
        fn not() {
            let mut vv: Values = [(1, Value::Active), (2, Value::Inactive), (3, Value::Active)]
                .into_iter()
                .collect();
            assert_eq!(vv.get(1), Some(Value::Active));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Active));
            vv.not();
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Active));
            assert_eq!(vv.get(3), Some(Value::Inactive));
            vv.not();
            assert_eq!(vv.get(1), Some(Value::Active));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Active));
        }

        #[test]
        fn from_offset_iterator() {
            let vv: Values = [1, 2, 3].iter().collect();
            assert_eq!(vv.get(1), Some(Value::Inactive));
            assert_eq!(vv.get(2), Some(Value::Inactive));
            assert_eq!(vv.get(3), Some(Value::Inactive));
        }

        #[test]
        fn from_line_value_iterator() {
            let vv: Values = [(1, Value::Active), (2, Value::Inactive), (3, Value::Active)]
                .into_iter()
                .collect();
            assert_eq!(vv.get(1), Some(Value::Active));
            assert_eq!(vv.get(2), Some(Value::Inactive));
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
            assert_eq!(
                i.next(),
                Some(&LineValue {
                    offset: 1,
                    value: Value::Inactive
                })
            );
            assert_eq!(
                i.next(),
                Some(&LineValue {
                    offset: 2,
                    value: Value::Active
                })
            );
            assert_eq!(
                i.next(),
                Some(&LineValue {
                    offset: 3,
                    value: Value::Inactive
                })
            );
            assert_eq!(i.next(), None);
        }

        #[test]
        fn iter_mut() {
            let mut vv = Values::from_offsets(&[1, 2, 3]);
            vv.set(2, Value::Active);
            let mut i = vv.iter_mut();
            // assumes keys returned in order...
            let mut lv = i.next();
            assert_eq!(
                lv,
                Some(&mut LineValue {
                    offset: 1,
                    value: Value::Inactive
                })
            );
            lv.expect("value should exist").value = Value::Active;
            lv = i.next();
            assert_eq!(
                lv,
                Some(&mut LineValue {
                    offset: 2,
                    value: Value::Active
                })
            );
            lv.expect("value should exist").value = Value::Inactive;
            lv = i.next();
            assert_eq!(
                lv,
                Some(&mut LineValue {
                    offset: 3,
                    value: Value::Inactive
                })
            );
            lv.expect("value should exist").value = Value::Active;
            assert_eq!(i.next(), None);
            assert_eq!(
                vv.iter().map(|lv| lv.value).collect::<Vec<Value>>(),
                [Value::Active, Value::Inactive, Value::Active]
            )
        }

        #[test]
        #[cfg(feature = "uapi_v1")]
        fn contains_keys() {
            let vv = Values::from_offsets(&[1, 2, 3]);
            assert!(!vv.contains_keys(&[0]));
            assert!(vv.contains_keys(&[1]));
            assert!(vv.contains_keys(&[2]));
            assert!(vv.contains_keys(&[3]));
            assert!(!vv.contains_keys(&[4]));
            assert!(vv.contains_keys(&[1, 2, 3]));
            assert!(vv.contains_keys(&[1, 3]));
            assert!(vv.contains_keys(&[2, 3]));
        }
    }
}
