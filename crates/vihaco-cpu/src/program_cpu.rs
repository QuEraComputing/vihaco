// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[path = "heap.rs"]
mod heap;

use heap::Heap;

use crate::Word;

#[vihaco_runtime::component]
#[instructions { crate::cpu }]
#[derive(Default, Debug)]
pub struct CPU {
    pub(crate) heap: Heap,
}

impl CPU {
    /// Borrow the stored words of the heap object with the given ID, in order.
    ///
    /// # Errors
    ///
    /// Returns an error if the ID is invalid or the object has been deallocated.
    pub fn heap_object(&self, id: u32) -> eyre::Result<&[Word]> {
        self.heap.get(id)
    }
}

impl vihaco_abi::traits::Reset for CPU {
    fn reset(&mut self) {
        self.heap.clear();
    }
}
