// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Resolved struct layouts carried by `struct_new`, `get_field`, and `set_field`.
//!
//! Lowering resolves struct and field names (or numeric references) against
//! the module's struct table and stores the result in the instruction, so the
//! CPU can check heap objects at runtime without consulting the table.

use std::fmt;

/// A struct-table entry and its field count, as allocated by `struct_new`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StructLayout {
    /// Index into the module's struct table.
    pub struct_id: u32,
    /// Number of fields, which is the number of operands `struct_new` pops.
    pub field_count: u32,
}

/// A field slot within a specific struct, as accessed by `get_field`/`set_field`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FieldSlot {
    /// Index into the module's struct table; the object must be this struct.
    pub struct_id: u32,
    /// Slot index of the field, in declaration order.
    pub slot: u32,
}

impl fmt::Display for StructLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "struct {} ({} fields)", self.struct_id, self.field_count)
    }
}

impl fmt::Display for FieldSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "struct {} field {}", self.struct_id, self.slot)
    }
}
