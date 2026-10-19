// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[path = "heap.rs"]
mod heap;

use heap::Heap;

#[vihaco_runtime::component]
#[instructions { crate::cpu }]
#[derive(Default, Debug)]
pub struct CPU {
    pub(crate) heap: Heap,
}

impl vihaco_abi::traits::Reset for CPU {
    fn reset(&mut self) {
        self.heap.clear();
    }
}
