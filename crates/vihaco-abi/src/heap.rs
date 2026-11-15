// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Runtime heap errors, as structured data.
//!
//! Heap references are generational keys packed into one word: the low 32
//! bits are the slot index and the high 32 bits its generation, which
//! advances every time the slot is freed. [`HeapRef`] shows a reference word
//! in those terms rather than as a raw number. [`HeapError`] carries the
//! types involved as [`Type`] values, so a host that has the module can show
//! them by name with [`HeapError::display_with`].

use std::fmt;

use crate::Type;
use crate::names::TypeNames;

/// A heap reference word, displayed as its slot and generation:
/// `(slot 1, generation 1)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HeapRef(pub u64);

impl HeapRef {
    /// The slot index: the low 32 bits.
    pub fn slot(self) -> u32 {
        self.0 as u32
    }

    /// The slot's generation: the high 32 bits.
    pub fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }
}

impl fmt::Display for HeapRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "(slot {}, generation {})",
            self.slot(),
            self.generation()
        )
    }
}

/// A failed heap access.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HeapError {
    /// No live object has this reference: it was never issued, or the object
    /// has been freed.
    InvalidReference {
        /// The reference used.
        reference: HeapRef,
    },
    /// The object is not of the type the instruction names.
    TypeMismatch {
        /// The reference used.
        reference: HeapRef,
        /// What the object is: `struct N` or `array T`.
        found: Type,
        /// What the instruction expects: `struct N` or `array T`.
        expected: Type,
    },
    /// The instruction accepts any array, but the object is not one.
    NotAnArray {
        /// The reference used.
        reference: HeapRef,
        /// What the object is.
        found: Type,
    },
    /// An append to an array already at its capacity.
    Full {
        /// The reference used.
        reference: HeapRef,
        /// The array's fixed capacity.
        capacity: usize,
    },
    /// An element index at or past the array's length.
    IndexOutOfBounds {
        /// The index used.
        index: u64,
        /// The array's current length.
        len: usize,
    },
    /// A field slot past the end of a struct object.
    NoSuchField {
        /// The struct-table index of the object.
        struct_id: u32,
        /// The field slot used.
        slot: u32,
        /// The number of fields the object has.
        field_count: usize,
    },
}

impl HeapError {
    /// Display this error with struct and named types shown by name (see
    /// [`Type::display_with`]).
    pub fn display_with<'a>(&'a self, names: &'a dyn TypeNames) -> impl fmt::Display + 'a {
        WithNames { error: self, names }
    }

    fn write(&self, f: &mut fmt::Formatter<'_>, names: &dyn TypeNames) -> fmt::Result {
        match self {
            HeapError::InvalidReference { reference } => write!(
                f,
                "heap reference {reference} is invalid or has been deallocated"
            ),
            HeapError::TypeMismatch {
                reference,
                found,
                expected,
            } => write!(
                f,
                "type error: heap object {reference} is {}, expected {}",
                found.display_with(names),
                expected.display_with(names)
            ),
            HeapError::NotAnArray { reference, found } => write!(
                f,
                "type error: heap object {reference} is {}, expected an array",
                found.display_with(names)
            ),
            HeapError::Full {
                reference,
                capacity,
            } => write!(f, "heap object {reference} is full (capacity {capacity})"),
            HeapError::IndexOutOfBounds { index, len } => {
                write!(f, "array index {index} out of bounds (length {len})")
            }
            HeapError::NoSuchField {
                struct_id,
                slot,
                field_count,
            } => write!(
                f,
                "{} has no field slot {slot} ({field_count} fields)",
                Type::Struct(*struct_id).display_with(names)
            ),
        }
    }
}

struct WithNames<'a> {
    error: &'a HeapError,
    names: &'a dyn TypeNames,
}

impl fmt::Display for WithNames<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.write(f, self.names)
    }
}

impl fmt::Display for HeapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write(f, &())
    }
}

impl std::error::Error for HeapError {}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tables;

    impl TypeNames for Tables {
        fn struct_name(&self, index: u32) -> Option<&str> {
            ["Point", "Pair"].get(index as usize).copied()
        }

        fn named_type_name(&self, _: u32) -> Option<&str> {
            None
        }
    }

    #[test]
    fn references_show_slot_and_generation() {
        let reference = HeapRef(0x1_0000_0001);
        assert_eq!((reference.slot(), reference.generation()), (1, 1));
        assert_eq!(reference.to_string(), "(slot 1, generation 1)");
    }

    #[test]
    fn errors_show_types_by_name_when_given_names() {
        let error = HeapError::TypeMismatch {
            reference: HeapRef(0x1_0000_0001),
            found: Type::Struct(1),
            expected: Type::Struct(0),
        };
        assert_eq!(
            error.to_string(),
            "type error: heap object (slot 1, generation 1) is struct 1, expected struct 0"
        );
        assert_eq!(
            error.display_with(&Tables).to_string(),
            "type error: heap object (slot 1, generation 1) is struct Pair, expected struct Point"
        );
        let error = HeapError::NoSuchField {
            struct_id: 0,
            slot: 4,
            field_count: 2,
        };
        assert_eq!(
            error.display_with(&Tables).to_string(),
            "struct Point has no field slot 4 (2 fields)"
        );
    }
}
