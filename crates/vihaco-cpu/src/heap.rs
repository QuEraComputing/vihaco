// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! CPU heap: type-tagged objects behind generational references.
//!
//! References are `slotmap` keys packed into a 64-bit word (32-bit slot index,
//! 32-bit version). Freeing an object bumps its slot's version, so a stale
//! reference is rejected rather than reaching the object that later reuses the
//! slot. The version is 32 bits and wraps, so this holds for up to 2^31 reuses
//! of one slot; a reference held across that many free/alloc cycles of its
//! slot would become valid again.
//!
//! References are not secret: a fresh heap always issues the same sequence
//! (its first object is slot 1, version 1, i.e. `0x1_0000_0001`), so a word can
//! be predicted. Memory safety does not depend on that: every access names the
//! kind of object it expects and is checked against the object's tag. Typed
//! dialects also have no instruction that writes a reference literal, so a
//! typed program can only use references it was given or allocated itself.
//!
//! Every object records what it is: an array with a specific element type, or
//! a specific struct. Each access names what it expects and is checked against
//! that tag before any element or field is touched.

use crate::Word;
use crate::layout::{FieldSlot, StructLayout};
use slotmap::{DefaultKey, Key, KeyData, SlotMap};
use vihaco_abi::Type;

/// What a heap object is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ObjectKind {
    /// Created by `array_new` or `array_reserve`; elements of the given type,
    /// accessed by index.
    Array(Type),
    /// Created by `struct_new`; a fixed set of fields, accessed by field slot.
    Struct(u32),
}

impl std::fmt::Display for ObjectKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectKind::Array(elem) => write!(f, "array {elem}"),
            ObjectKind::Struct(id) => write!(f, "struct {id}"),
        }
    }
}

/// Storage with a fixed capacity limit and a current length.
/// `array_new` creates a full array, `array_reserve` an empty one, and
/// `struct_new` a full struct. Appends never overwrite elements or grow the
/// allocation, and only arrays accept them. Reads and writes are bounded by
/// `values.len()` rather than capacity, so a write never extends an object.
#[derive(Debug)]
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
            kind: self.kind.clone(),
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
    /// Allocate a full array of `elem` values; spare Vec capacity does not
    /// permit appending.
    pub(crate) fn alloc_array(&mut self, elem: Type, values: impl Into<Vec<Word>>) -> HeapRef {
        let values = values.into();
        let capacity = values.len();
        self.insert(HeapObject {
            kind: ObjectKind::Array(elem),
            values,
            capacity,
        })
    }

    /// Allocate an empty array of `elem` values with an exact, fixed element
    /// limit. Returns an error if its storage cannot be allocated.
    pub(crate) fn reserve_array(&mut self, elem: Type, capacity: usize) -> eyre::Result<HeapRef> {
        let mut values = Vec::new();
        values.try_reserve_exact(capacity)?;
        Ok(self.insert(HeapObject {
            kind: ObjectKind::Array(elem),
            values,
            capacity,
        }))
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

    /// Append to an `elem` array without reallocating or overwriting elements.
    /// Returns an error for invalid, full, or wrongly typed objects.
    pub(crate) fn push(
        &mut self,
        reference: HeapRef,
        elem: &Type,
        value: Word,
    ) -> eyre::Result<()> {
        let object = self.object_mut(reference)?;
        check_array(reference, object, elem)?;
        eyre::ensure!(
            object.values.len() < object.capacity,
            "heap object {reference:#x} is full (capacity {})",
            object.capacity
        );
        object.values.push(value);
        Ok(())
    }

    /// Read element `index` of an `elem` array.
    pub(crate) fn get(&self, reference: HeapRef, elem: &Type, index: u64) -> eyre::Result<Word> {
        let object = self.object(reference)?;
        check_array(reference, object, elem)?;
        let slot = element(object, index)?;
        Ok(object.values[slot])
    }

    /// Overwrite element `index` of an `elem` array. The index must be below
    /// the current length; writes never extend an array.
    pub(crate) fn set(
        &mut self,
        reference: HeapRef,
        elem: &Type,
        index: u64,
        value: Word,
    ) -> eyre::Result<()> {
        let object = self.object_mut(reference)?;
        check_array(reference, object, elem)?;
        let slot = element(object, index)?;
        object.values[slot] = value;
        Ok(())
    }

    /// The current length of an array of any element type.
    pub(crate) fn len(&self, reference: HeapRef) -> eyre::Result<usize> {
        let object = self.object(reference)?;
        eyre::ensure!(
            matches!(object.kind, ObjectKind::Array(_)),
            "type error: heap object {reference:#x} is {}, expected an array",
            object.kind
        );
        Ok(object.values.len())
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

    /// Free an object of any kind. Freeing twice, or freeing through a
    /// reference whose slot has since been reused, is an error.
    pub(crate) fn dealloc(&mut self, reference: HeapRef) -> eyre::Result<()> {
        key(reference)
            .and_then(|key| self.objects.remove(key))
            .map(drop)
            .ok_or_else(|| invalid(reference))
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

    /// The raw elements of an array of any element type, for inspection.
    #[cfg(all(test, not(feature = "typed-declarations")))]
    pub(crate) fn elements(&self, reference: HeapRef) -> eyre::Result<&[Word]> {
        self.len(reference)?;
        Ok(&self.object(reference)?.values)
    }

    #[cfg(all(test, not(feature = "program-image-execution")))]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

fn check_array(reference: HeapRef, object: &HeapObject, elem: &Type) -> eyre::Result<()> {
    match &object.kind {
        ObjectKind::Array(actual) if actual == elem => Ok(()),
        kind => Err(eyre::eyre!(
            "type error: heap object {reference:#x} is {kind}, expected array {elem}"
        )),
    }
}

/// Bounds-check an element index against an array's current length.
fn element(object: &HeapObject, index: u64) -> eyre::Result<usize> {
    usize::try_from(index)
        .ok()
        .filter(|&slot| slot < object.values.len())
        .ok_or_else(|| {
            eyre::eyre!(
                "array index {index} out of bounds (length {})",
                object.values.len()
            )
        })
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
    fn allocated_arrays_are_full_even_with_spare_vec_capacity() {
        let mut heap = Heap::default();
        let mut values = Vec::with_capacity(8);
        values.push(1);
        let id = heap.alloc_array(Type::U64, values);
        let err = heap.push(id, &Type::U64, 2).unwrap_err().to_string();
        assert!(err.contains("full"), "{err}");
        assert_eq!(heap.get(id, &Type::U64, 0).unwrap(), 1);
    }

    #[test]
    fn array_elements_are_accessed_only_through_their_element_type() {
        let mut heap = Heap::default();
        let floats = heap.alloc_array(Type::F64, vec![1, 2, 3]);
        heap.set(floats, &Type::F64, 1, 5).unwrap();
        assert_eq!(heap.get(floats, &Type::F64, 1).unwrap(), 5);
        assert_eq!(heap.len(floats).unwrap(), 3);

        for err in [
            heap.get(floats, &Type::U64, 0).unwrap_err(),
            heap.set(floats, &Type::I64, 0, 1).unwrap_err(),
            heap.push(floats, &Type::array(Type::F64), 1).unwrap_err(),
        ] {
            assert!(
                err.to_string().contains("is array f64, expected array"),
                "{err}"
            );
        }
        let err = heap.get(floats, &Type::F64, 3).unwrap_err().to_string();
        assert!(
            err.contains("array index 3 out of bounds (length 3)"),
            "{err}"
        );
        assert!(heap.set(floats, &Type::F64, u64::MAX, 0).is_err());

        // Nested element types must match exactly.
        let nested = heap.alloc_array(Type::array(Type::U64), vec![floats]);
        assert!(heap.get(nested, &Type::array(Type::U64), 0).is_ok());
        let err = heap
            .get(nested, &Type::array(Type::F64), 0)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("is array array u64, expected array array f64"),
            "{err}"
        );
    }

    #[test]
    fn reserved_arrays_accept_appends_up_to_capacity_then_writes_within_length() {
        let mut heap = Heap::default();
        let id = heap.reserve_array(Type::I64, 2).unwrap();
        assert!(heap.set(id, &Type::I64, 0, 1).is_err(), "set never extends");
        heap.push(id, &Type::I64, 7).unwrap();
        heap.set(id, &Type::I64, 0, 8).unwrap();
        heap.push(id, &Type::I64, 9).unwrap();
        assert!(heap.push(id, &Type::I64, 10).is_err());
        assert_eq!(heap.len(id).unwrap(), 2);
        assert_eq!(heap.get(id, &Type::I64, 0).unwrap(), 8);
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
        let err = heap.get(point, &Type::U64, 0).unwrap_err().to_string();
        assert!(err.contains("is struct 0, expected array u64"), "{err}");
        let err = heap.len(point).unwrap_err().to_string();
        assert!(err.contains("is struct 0, expected an array"), "{err}");
        assert!(heap.push(point, &Type::U64, 3).is_err());

        // Arrays are not structs, even when the index would be in bounds.
        let array = heap.alloc_array(Type::U64, vec![1, 2]);
        let err = heap.field(array, field(0, 0)).unwrap_err().to_string();
        assert!(err.contains("is array u64, expected struct 0"), "{err}");
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
        let array = heap.alloc_array(Type::U64, vec![1]);
        heap.dealloc(array).unwrap();
        let err = heap.dealloc(array).unwrap_err().to_string();
        assert!(err.contains("invalid or has been deallocated"), "{err}");
        // Small literal numbers are not valid references.
        let live = heap.alloc_array(Type::U64, vec![1]);
        for forged in [0, 1, live as u32 as u64, u64::MAX] {
            assert!(heap.len(forged).is_err(), "{forged:#x}");
        }
    }

    /// Pins the reference policy: references are predictable, not secret. A
    /// fresh heap's first object is always `0x1_0000_0001`, so that word reaches
    /// it in any heap. What protects objects is the tag check (a predicted word
    /// still has to name the right kind of object) and, in typed dialects, the
    /// absence of any instruction that writes a reference literal.
    #[test]
    fn fresh_references_are_predictable_but_tag_checked() {
        const FIRST: HeapRef = 1 << 32 | 1;
        let mut a = Heap::default();
        let mut b = Heap::default();
        assert_eq!(a.alloc_array(Type::U64, vec![7]), FIRST);
        assert_eq!(b.alloc_struct(POINT, vec![1, 2]).unwrap(), FIRST);
        assert_eq!(a.get(FIRST, &Type::U64, 0).unwrap(), 7);
        // The same predicted word must still name the right kind of object.
        assert!(a.field(FIRST, field(0, 0)).is_err());
        assert!(b.get(FIRST, &Type::U64, 0).is_err());
        // Once freed, the predicted word is stale like any other reference.
        a.dealloc(FIRST).unwrap();
        let next = a.alloc_array(Type::U64, vec![8]);
        assert_ne!(next, FIRST);
        assert!(a.get(FIRST, &Type::U64, 0).is_err());
    }

    #[test]
    fn slot_index_alone_never_reaches_a_first_generation_object() {
        // The first object in a fresh heap has the lowest version; a forged
        // word holding only its slot index must still be rejected.
        let mut heap = Heap::default();
        let first = heap.alloc_array(Type::U64, vec![7]);
        let slot_only = first & 0xffff_ffff;
        assert_ne!(first, slot_only);
        assert!(heap.len(slot_only).is_err());
        assert!(heap.get(slot_only, &Type::U64, 0).is_err());
        assert!(heap.dealloc(slot_only).is_err());
        assert_eq!(heap.get(first, &Type::U64, 0).unwrap(), 7);
    }

    #[test]
    fn references_from_before_a_clear_stay_invalid() {
        let mut heap = Heap::default();
        let old = heap.alloc_array(Type::U64, vec![1]);
        heap.clear();
        let new = heap.alloc_array(Type::U64, vec![2]);
        assert!(heap.len(old).is_err());
        assert_eq!(heap.get(new, &Type::U64, 0).unwrap(), 2);
    }

    #[test]
    fn cloned_heap_preserves_append_capacity_without_reallocation() {
        let mut heap = Heap::default();
        let id = heap.reserve_array(Type::U64, 3).unwrap();
        heap.push(id, &Type::U64, 1).unwrap();
        let mut cloned = heap.clone();
        for heap in [&mut heap, &mut cloned] {
            let storage = heap.objects[key(id).unwrap()].values.as_ptr();
            heap.push(id, &Type::U64, 2).unwrap();
            heap.push(id, &Type::U64, 3).unwrap();
            assert_eq!(heap.objects[key(id).unwrap()].values.as_ptr(), storage);
            assert_eq!(heap.len(id).unwrap(), 3);
            assert!(heap.push(id, &Type::U64, 4).is_err());
        }
    }

    #[test]
    fn cloned_heap_preserves_object_kinds_and_references() {
        let mut heap = Heap::default();
        let point = heap.alloc_struct(POINT, vec![1, 2]).unwrap();
        let floats = heap.alloc_array(Type::F64, vec![9]);
        let cloned = heap.clone();
        assert_eq!(cloned.field(point, field(0, 1)).unwrap(), 2);
        assert!(cloned.len(point).is_err());
        assert_eq!(cloned.get(floats, &Type::F64, 0).unwrap(), 9);
        assert!(cloned.get(floats, &Type::U64, 0).is_err());
    }
}
