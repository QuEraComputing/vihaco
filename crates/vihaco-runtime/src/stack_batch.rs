// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::ops::Deref;

use smallvec::SmallVec;

/// Owned operands in their original stack order.
///
/// Up to eight values stay inline. Larger batches use heap storage. The backing
/// collection and capacity are implementation details; receiving a batch never
/// borrows program storage. Components can retain it after execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackBatch<T> {
    pub(crate) values: SmallVec<[T; 8]>,
}

impl<T> StackBatch<T> {
    pub fn as_slice(&self) -> &[T] {
        self.values.as_slice()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl<T> AsRef<[T]> for StackBatch<T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> Deref for StackBatch<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

/// Consuming iterator over a [`StackBatch`], without materializing another buffer.
pub struct StackBatchIntoIter<T>(smallvec::IntoIter<[T; 8]>);

impl<T> Iterator for StackBatchIntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<T> DoubleEndedIterator for StackBatchIntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        self.0.next_back()
    }
}

impl<T> ExactSizeIterator for StackBatchIntoIter<T> {}

impl<T> std::iter::FusedIterator for StackBatchIntoIter<T> {}

impl<T> IntoIterator for StackBatch<T> {
    type Item = T;
    type IntoIter = StackBatchIntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        StackBatchIntoIter(self.values.into_iter())
    }
}

impl<'a, T> IntoIterator for &'a StackBatch<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}
