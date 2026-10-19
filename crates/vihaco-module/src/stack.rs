// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Typed access to individual logical stack values.
//!
//! These traits describe storage access, independently of a CPU, composite, or
//! backing representation. Available with `program-image-execution`.
//!
//! These are storage extension contracts for the framework, not requirements
//! for ordinary component/composite authors. `vihaco_stdlib::ExecutionState`
//! supplies access; instructions declare operand/result types.

use vihaco_abi::word::{
    Word, decode_bool, decode_i32, decode_i64, decode_u32, encode_bool, encode_i32, encode_i64,
    encode_u32,
};

/// Low-level typed operand access, excluding protected frame locals.
///
/// To be implemented by vihaco's execution storage. Component authors choose
/// `PopFromStack` messages instead of implementing stack access. This public
/// extension point also permits custom storage implementations.
///
/// The type parameter selects the representation: word-backed storage can
/// decode floats, while an optimized cursor can consume native float values.
pub trait PopStack<T> {
    /// Consume exactly one logical operand as `T`.
    ///
    /// # Errors
    /// Returns an error if no operand is available. Earlier successful reads
    /// remain consumed when a subsequent read fails.
    fn pop(&mut self) -> eyre::Result<T>;
}

/// Low-level typed insertion into logical stack storage.
///
/// To be implemented by vihaco's execution storage. Component authors return
/// `PushToStack` effects instead of implementing stack insertion. This public
/// extension point also permits custom storage implementations.
///
/// Each insertion adds exactly one value above existing values. Word-backed
/// implementations encode floats with [`vihaco_abi::word::encode_f32`] and
/// [`vihaco_abi::word::encode_f64`]; a
/// later cached representation can retain native floating-point values instead.
/// Floating-point insertion must preserve exact IEEE-754 bits.
pub trait PushStack<T> {
    /// Insert one value in this representation.
    fn push(&mut self, value: T);
}

/// One copied logical stack value and its storage-representation conversions.
///
/// Separates the type a component computes with from the representation used to
/// access program storage. A component can declare operands such as
/// `PopFromStack<(i64, f64)>` and receive ordinary Rust values, while
/// `vihaco_stdlib::ExecutionState` decides how to store them. A later optimized
/// cursor can keep a floating-point value in a native cache without changing
/// that component's messages or effects.
/// This trait is framework plumbing; ordinary component authors do not implement
/// it or any stack access logic.
///
/// Conversions describe one logical value and have no stack access. The default
/// readers/writers consume/produce exactly one value through those conversions.
/// Custom items need not have a Word encoding. This trait does not select an
/// output storage bank or require native float insertion to pass through Word.
///
/// Booleans decode zero as false and every nonzero word as true. Insertion
/// normalizes them to `0` or `1`; no canonical-input validation is performed.
pub trait StackValue: Copy {
    /// The value representation exchanged with the program's storage machinery.
    ///
    /// Built-in integers and booleans use [`Word`]; floats use their native
    /// types. For example, components compute with `bool`, while its storage
    /// access uses an encoded Word. An `f64` stays an `f64` at this interface:
    /// word-backed storage encodes its bits, while an optimized cursor can retain
    /// the native value and avoid conversions between instructions.
    ///
    /// "Storage" describes this internal access representation, not a physical
    /// memory location, allocation, or bytecode encoding. It lets program storage
    /// evolve independently of the component-facing operand/result types. The
    /// selected runtime cache bank remains the storage implementation's concern.
    type Storage;

    /// Convert one stored representation into a component-facing value.
    ///
    /// # Errors
    /// Returns any value-validation error required by a custom item. Built-in
    /// conversions accept every input, including truthy Boolean words.
    fn from_storage(value: Self::Storage) -> eyre::Result<Self>;

    /// Convert this value into the representation accepted by program storage.
    fn into_storage(self) -> Self::Storage;

    /// Consume one operand and convert it to this item.
    ///
    /// # Errors
    /// Propagates source-access or conversion errors. If conversion fails, the
    /// operand has already been consumed; previous pops are not rolled back.
    #[inline]
    fn pop_from(source: &mut impl PopStack<Self::Storage>) -> eyre::Result<Self> {
        source.pop().and_then(Self::from_storage)
    }

    /// Convert and insert this item using the destination's typed writer.
    #[inline]
    fn push_to(self, destination: &mut impl PushStack<Self::Storage>) {
        destination.push(self.into_storage());
    }
}

macro_rules! identity_item {
    ($ty:ty) => {
        impl StackValue for $ty {
            type Storage = Self;

            #[inline]
            fn from_storage(value: Self::Storage) -> eyre::Result<Self> {
                Ok(value)
            }

            #[inline]
            fn into_storage(self) -> Self::Storage {
                self
            }

            // Preserve direct Result forwarding for identity readers.
            #[inline]
            fn pop_from(source: &mut impl PopStack<Self::Storage>) -> eyre::Result<Self> {
                source.pop()
            }
        }
    };
}

identity_item!(Word);
identity_item!(f32);
identity_item!(f64);

macro_rules! word_item {
    ($ty:ty, $decode:path, $encode:path) => {
        impl StackValue for $ty {
            type Storage = Word;

            #[inline]
            fn from_storage(value: Self::Storage) -> eyre::Result<Self> {
                Ok($decode(value))
            }

            #[inline]
            fn into_storage(self) -> Self::Storage {
                $encode(self)
            }
        }
    };
}

word_item!(i32, decode_i32, encode_i32);
word_item!(i64, decode_i64, encode_i64);
word_item!(u32, decode_u32, encode_u32);
word_item!(bool, decode_bool, encode_bool);
