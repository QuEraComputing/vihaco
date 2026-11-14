// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! CPU heap: kind-tagged objects behind generational references.
//!
//! References are `slotmap` keys packed into a 64-bit word (32-bit slot index,
//! 32-bit version). Freeing an object bumps its slot's version, so a stale
//! reference can never reach the object that later reuses the slot.

use crate::Word;
use crate::layout::{FieldSlot, StructLayout};
use slotmap::{DefaultKey, Key, KeyData, SlotMap};

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

/// A heap reference as it appears in a word: the packed generational key.
pub(crate) type HeapRef = u64;

/// Cloning preserves every key, so references stay valid in the clone.
#[derive(Debug, Clone, Default)]
pub(crate) struct Heap {
    objects: SlotMap<DefaultKey, HeapObject>,
}

/// Decode a reference word into a key, or `None` if no live object could have it.
///
/// Issued references always carry an odd version in their high 32 bits.
/// `KeyData::from_ffi` would silently force an even version odd, turning a
/// forged small number such as `1` into a key for the first object allocated
/// in that slot, so such words are rejected here instead. This relies on the
/// slotmap 1.x key layout; `slot_index_alone_never_reaches_a_first_generation_object`
/// fails if that layout changes.
fn key(reference: HeapRef) -> Option<DefaultKey> {
    let version = (reference >> 32) as u32;
    (version & 1 == 1).then(|| KeyData::from_ffi(reference).into())
}

fn invalid(reference: HeapRef) -> eyre::Report {
    eyre::eyre!("heap reference {reference:#x} is invalid or has been deallocated")
}

impl Heap {
    /// Allocate a full array; spare Vec capacity does not permit appending.
    pub(crate) fn alloc(&mut self, values: impl Into<Vec<Word>>) -> HeapRef {
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
    ) -> eyre::Result<HeapRef> {
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
    pub(crate) fn reserve(&mut self, capacity: usize) -> eyre::Result<HeapRef> {
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
    pub(crate) fn push(&mut self, reference: HeapRef, value: Word) -> eyre::Result<()> {
        let object = self.array_mut(reference)?;
        eyre::ensure!(
            object.values.len() < object.capacity,
            "heap object {reference:#x} is full (capacity {})",
            object.capacity
        );
        object.values.push(value);
        Ok(())
    }

    /// Read a field of a struct object.
    /// Returns an error unless `reference` is a live struct of type
    /// `field.struct_id` with a field at `field.slot`.
    pub(crate) fn field(&self, reference: HeapRef, field: FieldSlot) -> eyre::Result<Word> {
        let object = self.object(reference)?;
        check_struct(reference, object, field)?;
        Ok(object.values[field.slot as usize])
    }

    /// Overwrite a field of a struct object, with the same checks as [`Self::field`].
    pub(crate) fn set_field(
        &mut self,
        reference: HeapRef,
        field: FieldSlot,
        value: Word,
    ) -> eyre::Result<()> {
        let object = self.object_mut(reference)?;
        check_struct(reference, object, field)?;
        object.values[field.slot as usize] = value;
        Ok(())
    }

    fn insert(&mut self, object: HeapObject) -> HeapRef {
        self.objects.insert(object).data().as_ffi()
    }

    /// Free an object of either kind. Freeing twice, or freeing through a
    /// reference whose slot has since been reused, is an error.
    pub(crate) fn dealloc(&mut self, reference: HeapRef) -> eyre::Result<()> {
        key(reference)
            .and_then(|key| self.objects.remove(key))
            .map(drop)
            .ok_or_else(|| invalid(reference))
    }

    /// The elements of an array object. Struct objects are rejected: their
    /// fields are only reachable through checked field access.
    pub(crate) fn array(&self, reference: HeapRef) -> eyre::Result<&[Word]> {
        let object = self.object(reference)?;
        check_array(reference, object)?;
        Ok(&object.values)
    }

    fn array_mut(&mut self, reference: HeapRef) -> eyre::Result<&mut HeapObject> {
        let object = self.object_mut(reference)?;
        check_array(reference, object)?;
        Ok(object)
    }

    fn object(&self, reference: HeapRef) -> eyre::Result<&HeapObject> {
        key(reference)
            .and_then(|key| self.objects.get(key))
            .ok_or_else(|| invalid(reference))
    }

    fn object_mut(&mut self, reference: HeapRef) -> eyre::Result<&mut HeapObject> {
        key(reference)
            .and_then(|key| self.objects.get_mut(key))
            .ok_or_else(|| invalid(reference))
    }

    /// Free every object. Slot versions keep advancing, so references from
    /// before the clear stay invalid.
    pub(crate) fn clear(&mut self) {
        self.objects.clear();
    }

    #[cfg(all(test, not(feature = "program-image-execution")))]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

fn check_array(reference: HeapRef, object: &HeapObject) -> eyre::Result<()> {
    eyre::ensure!(
        object.kind == ObjectKind::Array,
        "type error: heap object {reference:#x} is {}, expected an array",
        object.kind
    );
    Ok(())
}

fn check_struct(reference: HeapRef, object: &HeapObject, field: FieldSlot) -> eyre::Result<()> {
    eyre::ensure!(
        object.kind == ObjectKind::Struct(field.struct_id),
        "type error: heap object {reference:#x} is {}, expected struct {}",
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
    fn stale_references_never_reach_an_object_that_reuses_their_slot() {
        let mut heap = Heap::default();
        let old = heap.alloc_struct(POINT, vec![1, 2]).unwrap();
        heap.dealloc(old).unwrap();
        // Same kind, same layout: only the generation tells them apart.
        let new = heap.alloc_struct(POINT, vec![3, 4]).unwrap();
        assert_ne!(old, new);
        assert_eq!(old as u32, new as u32, "the slot itself is reused");
        let err = heap.field(old, field(0, 0)).unwrap_err().to_string();
        assert!(err.contains("invalid or has been deallocated"), "{err}");
        assert!(heap.set_field(old, field(0, 0), 9).is_err());
        assert!(heap.dealloc(old).is_err());
        assert_eq!(heap.field(new, field(0, 0)).unwrap(), 3);
    }

    #[test]
    fn double_free_and_forged_references_are_rejected() {
        let mut heap = Heap::default();
        let array = heap.alloc(vec![1]);
        heap.dealloc(array).unwrap();
        let err = heap.dealloc(array).unwrap_err().to_string();
        assert!(err.contains("invalid or has been deallocated"), "{err}");
        // Small literal numbers are not valid references.
        let live = heap.alloc(vec![1]);
        for forged in [0, 1, live as u32 as u64, u64::MAX] {
            assert!(heap.array(forged).is_err(), "{forged:#x}");
        }
    }

    #[test]
    fn slot_index_alone_never_reaches_a_first_generation_object() {
        // The first object in a fresh heap has the lowest version; a forged
        // word holding only its slot index must still be rejected.
        let mut heap = Heap::default();
        let first = heap.alloc(vec![7]);
        let slot_only = first & 0xffff_ffff;
        assert_ne!(first, slot_only);
        assert!(heap.array(slot_only).is_err());
        assert!(heap.field(slot_only, field(0, 0)).is_err());
        assert!(heap.dealloc(slot_only).is_err());
        assert_eq!(heap.array(first).unwrap(), &[7]);
    }

    #[test]
    fn references_from_before_a_clear_stay_invalid() {
        let mut heap = Heap::default();
        let old = heap.alloc(vec![1]);
        heap.clear();
        let new = heap.alloc(vec![2]);
        assert!(heap.array(old).is_err());
        assert_eq!(heap.array(new).unwrap(), &[2]);
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
    fn cloned_heap_preserves_object_kinds_and_references() {
        let mut heap = Heap::default();
        let point = heap.alloc_struct(POINT, vec![1, 2]).unwrap();
        let cloned = heap.clone();
        assert_eq!(cloned.field(point, field(0, 1)).unwrap(), 2);
        assert!(cloned.array(point).is_err());
    }
}
