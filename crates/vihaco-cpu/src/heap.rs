// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::Word;
use crate::layout::{FieldSlot, StructLayout};

/// What a heap object is. Every access checks the kind before touching slots,
/// so struct and array operations cannot be applied to the wrong object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ObjectKind {
    /// Created by `heap_alloc` or `heap_reserve`; accessed by index.
    #[default]
    Array,
    /// Created by `struct_new`; a fixed set of fields, accessed by field slot.
    Struct(u32),
}

impl std::fmt::Display for ObjectKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectKind::Array => f.write_str("an array"),
            ObjectKind::Struct(id) => write!(f, "struct {id}"),
        }
    }
}

/// Storage with a fixed capacity limit and a current length.
/// `heap_alloc` creates a full array, `heap_reserve` an empty one, and
/// `struct_new` a full struct. Appends never overwrite elements or grow the
/// allocation, and only arrays accept them. Reads and field writes are bounded
/// by `values.len()` rather than capacity, so a write never extends an object.
#[derive(Debug, Default)]
struct HeapObject {
    kind: ObjectKind,
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
            kind: self.kind,
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
    /// Allocate a full array; spare Vec capacity does not permit appending.
    pub(crate) fn alloc(&mut self, values: impl Into<Vec<Word>>) -> u32 {
        let values = values.into();
        let capacity = values.len();
        self.insert(HeapObject {
            kind: ObjectKind::Array,
            values,
            capacity,
        })
    }

    /// Allocate a struct whose fields are `values`, in declaration order.
    /// Returns an error unless exactly `layout.field_count` values are supplied.
    pub(crate) fn alloc_struct(
        &mut self,
        layout: StructLayout,
        values: impl Into<Vec<Word>>,
    ) -> eyre::Result<u32> {
        let values = values.into();
        eyre::ensure!(
            values.len() == layout.field_count as usize,
            "struct {} has {} fields, but {} values were supplied",
            layout.struct_id,
            layout.field_count,
            values.len()
        );
        let capacity = values.len();
        Ok(self.insert(HeapObject {
            kind: ObjectKind::Struct(layout.struct_id),
            values,
            capacity,
        }))
    }

    /// Allocate an empty array with an exact, fixed element limit.
    /// Returns an error if its storage cannot be allocated.
    pub(crate) fn reserve(&mut self, capacity: usize) -> eyre::Result<u32> {
        let mut values = Vec::new();
        values.try_reserve_exact(capacity)?;
        Ok(self.insert(HeapObject {
            kind: ObjectKind::Array,
            values,
            capacity,
        }))
    }

    /// Append to an array without reallocating or overwriting existing elements.
    /// Returns an error for invalid, deallocated, full, or struct objects.
    pub(crate) fn push(&mut self, id: u32, value: Word) -> eyre::Result<()> {
        let object = self.array_mut(id)?;
        eyre::ensure!(
            object.values.len() < object.capacity,
            "heap object {} is full (capacity {})",
            id,
            object.capacity
        );
        object.values.push(value);
        Ok(())
    }

    /// Read a field of a struct object.
    /// Returns an error unless `id` is a live struct of type `field.struct_id`
    /// with a field at `field.slot`.
    pub(crate) fn field(&self, id: u32, field: FieldSlot) -> eyre::Result<Word> {
        let object = self.object(id)?;
        check_struct(id, object, field)?;
        Ok(object.values[field.slot as usize])
    }

    /// Overwrite a field of a struct object, with the same checks as [`Self::field`].
    pub(crate) fn set_field(&mut self, id: u32, field: FieldSlot, value: Word) -> eyre::Result<()> {
        let object = self.object_mut(id)?;
        check_struct(id, object, field)?;
        object.values[field.slot as usize] = value;
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

    /// Free an object of either kind.
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

    /// The elements of an array object. Struct objects are rejected: their
    /// fields are only reachable through checked field access.
    pub(crate) fn array(&self, id: u32) -> eyre::Result<&[Word]> {
        let object = self.object(id)?;
        check_array(id, object)?;
        Ok(&object.values)
    }

    fn array_mut(&mut self, id: u32) -> eyre::Result<&mut HeapObject> {
        let object = self.object_mut(id)?;
        check_array(id, object)?;
        Ok(object)
    }

    fn object(&self, id: u32) -> eyre::Result<&HeapObject> {
        match self.slots.get(id as usize) {
            Some(HeapSlot::Alive(object)) => Ok(object),
            Some(HeapSlot::Deallocated) => {
                Err(eyre::eyre!("heap object {} has been deallocated", id))
            }
            None => Err(eyre::eyre!("invalid heap object id {}", id)),
        }
    }

    fn object_mut(&mut self, id: u32) -> eyre::Result<&mut HeapObject> {
        match self.slots.get_mut(id as usize) {
            Some(HeapSlot::Alive(object)) => Ok(object),
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

fn check_array(id: u32, object: &HeapObject) -> eyre::Result<()> {
    eyre::ensure!(
        object.kind == ObjectKind::Array,
        "type error: heap object {id} is {}, expected an array",
        object.kind
    );
    Ok(())
}

fn check_struct(id: u32, object: &HeapObject, field: FieldSlot) -> eyre::Result<()> {
    eyre::ensure!(
        object.kind == ObjectKind::Struct(field.struct_id),
        "type error: heap object {id} is {}, expected struct {}",
        object.kind,
        field.struct_id
    );
    // Struct objects are allocated with exactly their field count and never
    // grow, so a slot beyond it means the instruction disagrees with the layout.
    eyre::ensure!(
        (field.slot as usize) < object.values.len(),
        "struct {} has no field slot {} ({} fields)",
        field.struct_id,
        field.slot,
        object.values.len()
    );
    Ok(())
}

#[cfg(test)]
mod heap_tests {
    use super::*;

    const POINT: StructLayout = StructLayout {
        struct_id: 0,
        field_count: 2,
    };

    fn field(struct_id: u32, slot: u32) -> FieldSlot {
        FieldSlot { struct_id, slot }
    }

    #[test]
    fn allocated_objects_are_full_even_with_spare_vec_capacity() {
        let mut heap = Heap::default();
        let mut values = Vec::with_capacity(8);
        values.push(1);
        let id = heap.alloc(values);
        assert!(heap.push(id, 2).unwrap_err().to_string().contains("full"));
        assert_eq!(heap.array(id).unwrap(), &[1]);
    }

    #[test]
    fn struct_fields_are_read_and_written_only_through_their_struct_type() {
        let mut heap = Heap::default();
        let point = heap.alloc_struct(POINT, vec![1, 2]).unwrap();
        heap.set_field(point, field(0, 1), 5).unwrap();
        assert_eq!(heap.field(point, field(0, 0)).unwrap(), 1);
        assert_eq!(heap.field(point, field(0, 1)).unwrap(), 5);

        let err = heap.field(point, field(1, 0)).unwrap_err().to_string();
        assert!(err.contains("is struct 0, expected struct 1"), "{err}");
        let err = heap
            .set_field(point, field(0, 2), 9)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no field slot 2"), "{err}");

        // Struct objects are not arrays: no indexed access, length, or appends.
        let err = heap.array(point).unwrap_err().to_string();
        assert!(err.contains("is struct 0, expected an array"), "{err}");
        assert!(heap.push(point, 3).is_err());

        // Arrays are not structs, even when the index would be in bounds.
        let array = heap.alloc(vec![1, 2]);
        let err = heap.field(array, field(0, 0)).unwrap_err().to_string();
        assert!(err.contains("is an array, expected struct 0"), "{err}");
        assert!(heap.set_field(array, field(0, 0), 1).is_err());

        heap.dealloc(point).unwrap();
        let err = heap.field(point, field(0, 0)).unwrap_err().to_string();
        assert!(err.contains("deallocated"), "{err}");
        assert!(heap.field(99, field(0, 0)).is_err());
    }

    #[test]
    fn struct_allocation_requires_exactly_the_layout_field_count() {
        let mut heap = Heap::default();
        let err = heap.alloc_struct(POINT, vec![1]).unwrap_err().to_string();
        assert!(err.contains("has 2 fields, but 1 values"), "{err}");
        assert!(heap.alloc_struct(POINT, vec![1, 2, 3]).is_err());
        let empty = StructLayout {
            struct_id: 3,
            field_count: 0,
        };
        let unit = heap.alloc_struct(empty, Vec::new()).unwrap();
        assert!(heap.field(unit, field(3, 0)).is_err());
    }

    #[test]
    fn freed_struct_slots_are_reused_with_their_new_kind() {
        let mut heap = Heap::default();
        let point = heap.alloc_struct(POINT, vec![1, 2]).unwrap();
        heap.dealloc(point).unwrap();
        let array = heap.alloc(vec![1, 2]);
        assert_eq!(array, point);
        assert!(heap.field(array, field(0, 0)).is_err());
        assert_eq!(heap.array(array).unwrap(), &[1, 2]);
    }

    #[test]
    fn cloned_heap_preserves_append_capacity_without_reallocation() {
        let mut heap = Heap::default();
        let id = heap.reserve(3).unwrap();
        heap.push(id, 1).unwrap();
        let mut cloned = heap.clone();
        for heap in [&mut heap, &mut cloned] {
            let storage = heap.array(id).unwrap().as_ptr();
            heap.push(id, 2).unwrap();
            heap.push(id, 3).unwrap();
            assert_eq!(heap.array(id).unwrap().as_ptr(), storage);
            assert_eq!(heap.array(id).unwrap(), &[1, 2, 3]);
            assert!(heap.push(id, 4).is_err());
        }
    }

    #[test]
    fn cloned_heap_preserves_object_kinds() {
        let mut heap = Heap::default();
        let point = heap.alloc_struct(POINT, vec![1, 2]).unwrap();
        let cloned = heap.clone();
        assert_eq!(cloned.field(point, field(0, 1)).unwrap(), 2);
        assert!(cloned.array(point).is_err());
    }
}
