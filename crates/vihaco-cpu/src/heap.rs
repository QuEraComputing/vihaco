// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::Word;

/// Append-only storage with a fixed capacity limit and a current length.
/// `heap_alloc` creates a full object, and `heap_reserve` creates an empty one.
/// Appends never overwrite elements or grow the allocation, and reads are
/// bounded by `values.len()` rather than capacity.
#[derive(Debug, Default)]
struct HeapObject {
    values: Vec<Word>,
    capacity: usize,
}

impl Clone for HeapObject {
    fn clone(&self) -> Self {
        // Vec::clone may discard spare capacity. Preserve it so appending to a
        // cloned CPU's heap also needs no allocation.
        let mut values = Vec::with_capacity(self.capacity);
        values.extend_from_slice(&self.values);
        Self {
            values,
            capacity: self.capacity,
        }
    }
}

#[derive(Debug, Clone)]
enum HeapSlot {
    Alive(HeapObject),
    Deallocated,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Heap {
    slots: Vec<HeapSlot>,
    free_list: Vec<u32>,
}

impl Heap {
    /// Allocate a full object; spare Vec capacity does not permit appending.
    pub(crate) fn alloc(&mut self, values: impl Into<Vec<Word>>) -> u32 {
        let values = values.into();
        let capacity = values.len();
        self.insert(HeapObject { values, capacity })
    }

    /// Allocate an empty object with an exact, fixed element limit.
    /// Returns an error if its storage cannot be allocated.
    pub(crate) fn reserve(&mut self, capacity: usize) -> eyre::Result<u32> {
        let mut values = Vec::new();
        values.try_reserve_exact(capacity)?;
        Ok(self.insert(HeapObject { values, capacity }))
    }

    /// Append without reallocating or overwriting existing elements.
    /// Returns an error for invalid, deallocated, or full objects.
    pub(crate) fn push(&mut self, id: u32, value: Word) -> eyre::Result<()> {
        let object = match self.slots.get_mut(id as usize) {
            Some(HeapSlot::Alive(object)) => object,
            Some(HeapSlot::Deallocated) => {
                return Err(eyre::eyre!("heap object {} has been deallocated", id));
            }
            None => return Err(eyre::eyre!("invalid heap object id {}", id)),
        };
        eyre::ensure!(
            object.values.len() < object.capacity,
            "heap object {} is full (capacity {})",
            id,
            object.capacity
        );
        object.values.push(value);
        Ok(())
    }

    fn insert(&mut self, object: HeapObject) -> u32 {
        if let Some(id) = self.free_list.pop() {
            self.slots[id as usize] = HeapSlot::Alive(object);
            id
        } else {
            let id = self.slots.len() as u32;
            self.slots.push(HeapSlot::Alive(object));
            id
        }
    }

    pub(crate) fn dealloc(&mut self, id: u32) -> eyre::Result<()> {
        match self.slots.get_mut(id as usize) {
            Some(slot @ HeapSlot::Alive(_)) => {
                *slot = HeapSlot::Deallocated;
                self.free_list.push(id);
                Ok(())
            }
            Some(HeapSlot::Deallocated) => Err(eyre::eyre!(
                "double-free: heap object {} already deallocated",
                id
            )),
            None => Err(eyre::eyre!("invalid heap object id {}", id)),
        }
    }

    pub(crate) fn get(&self, id: u32) -> eyre::Result<&[Word]> {
        match self.slots.get(id as usize) {
            Some(HeapSlot::Alive(object)) => Ok(&object.values),
            Some(HeapSlot::Deallocated) => {
                Err(eyre::eyre!("heap object {} has been deallocated", id))
            }
            None => Err(eyre::eyre!("invalid heap object id {}", id)),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.slots.clear();
        self.free_list.clear();
    }

    #[cfg(all(test, not(feature = "program-image-execution")))]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

#[cfg(test)]
mod heap_tests {
    use super::Heap;

    #[test]
    fn allocated_objects_are_full_even_with_spare_vec_capacity() {
        let mut heap = Heap::default();
        let mut values = Vec::with_capacity(8);
        values.push(1);
        let id = heap.alloc(values);
        assert!(heap.push(id, 2).unwrap_err().to_string().contains("full"));
        assert_eq!(heap.get(id).unwrap(), &[1]);
    }

    #[test]
    fn cloned_heap_preserves_append_capacity_without_reallocation() {
        let mut heap = Heap::default();
        let id = heap.reserve(3).unwrap();
        heap.push(id, 1).unwrap();
        let mut cloned = heap.clone();
        for heap in [&mut heap, &mut cloned] {
            let storage = heap.get(id).unwrap().as_ptr();
            heap.push(id, 2).unwrap();
            heap.push(id, 3).unwrap();
            assert_eq!(heap.get(id).unwrap().as_ptr(), storage);
            assert_eq!(heap.get(id).unwrap(), &[1, 2, 3]);
            assert!(heap.push(id, 4).is_err());
        }
    }
}
