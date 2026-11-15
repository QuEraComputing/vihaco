// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Abstract values: the [`Lattice`] and [`TypedSlot`] traits and the standard
//! [`TypeSlot`].

use std::fmt;

use vihaco_abi::Type;

use crate::diagnostic::VerifyError;

/// A finite-height join semilattice of abstract values.
///
/// The engine joins the states that reach a leader along different edges and
/// requeues the leader only when its saved state strictly changes, so the
/// fixpoint terminates exactly when every chain of strict joins is finite.
pub trait Lattice: Clone + PartialEq + fmt::Display {
    /// An upper bound of `self` and `other`.
    ///
    /// The engine needs only two properties: the result is at least as
    /// general as `self` (joining is monotone), and joins have finite height
    /// (repeatedly joining new values into a saved value may change it only
    /// finitely many times). It need not be the least upper bound, nor
    /// commutative: [`TypeSlot`] keeps the first two conflicting types it saw.
    fn join(&self, other: &Self) -> Self;
}

/// Slots that can represent vihaco's [`Type`]s.
///
/// Standard rules bound only on this trait, so they work with any embedder's
/// slot type, including ones that track extra facts on top of a type.
pub trait TypedSlot: Lattice {
    /// The slot for a value known to have type `ty`.
    fn of_type(ty: Type) -> Self;

    /// Accept this slot where a value of type `expected` is required, or
    /// report a [`DiagnosticKind::TypeMismatch`](crate::DiagnosticKind::TypeMismatch).
    fn expect(&self, expected: &Type) -> Result<(), VerifyError>;

    /// The type every path agrees this slot has, or `None` (for example after
    /// a conflict). Rules that accept a family of types, such as any
    /// `array T`, inspect it.
    fn known_type(&self) -> Option<&Type>;
}

/// Whether a value of type `found` may be used where `expected` is required.
///
/// Types are compatible when they are equal; in addition, `struct N` and
/// `array T` satisfy `heap`, so instructions that accept any heap object accept
/// both. `heap` satisfies neither `struct N` nor `array T`. A named type
/// `type N` is nominal and satisfies only itself: not its representation,
/// another named type, or `heap`.
pub fn type_satisfies(found: &Type, expected: &Type) -> bool {
    found == expected
        || (matches!(expected, Type::HeapRef) && matches!(found, Type::Struct(_) | Type::Array(_)))
}

/// The standard slot: a vihaco [`Type`], or a conflict between two paths.
///
/// Joining two equal known types keeps the type; joining unequal known types
/// yields a [`Conflict`](TypeSlot::Conflict), which absorbs every later join.
/// A conflict never satisfies [`TypedSlot::expect`], so a value whose type
/// depends on the path taken cannot be used where a type is required; a
/// conflict that is never used is harmless.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeSlot {
    /// Every path agrees the value has this type.
    Known(Type),
    /// Paths disagreed at a join: the first two types seen.
    Conflict(Type, Type),
}

impl Lattice for TypeSlot {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (TypeSlot::Known(a), TypeSlot::Known(b)) if a == b => self.clone(),
            (TypeSlot::Known(a), TypeSlot::Known(b)) => TypeSlot::Conflict(a.clone(), b.clone()),
            (TypeSlot::Conflict(..), _) => self.clone(),
            (TypeSlot::Known(_), TypeSlot::Conflict(..)) => other.clone(),
        }
    }
}

impl TypedSlot for TypeSlot {
    fn of_type(ty: Type) -> Self {
        TypeSlot::Known(ty)
    }

    fn expect(&self, expected: &Type) -> Result<(), VerifyError> {
        match self {
            TypeSlot::Known(found) if type_satisfies(found, expected) => Ok(()),
            _ => Err(VerifyError::type_mismatch(expected, self)),
        }
    }

    fn known_type(&self) -> Option<&Type> {
        match self {
            TypeSlot::Known(ty) => Some(ty),
            TypeSlot::Conflict(..) => None,
        }
    }
}

impl From<Type> for TypeSlot {
    fn from(ty: Type) -> Self {
        TypeSlot::Known(ty)
    }
}

impl fmt::Display for TypeSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeSlot::Known(ty) => write!(f, "{ty}"),
            TypeSlot::Conflict(a, b) => write!(f, "{a} | {b}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagnosticKind;

    #[test]
    fn heap_objects_satisfy_heap_but_not_the_reverse() {
        assert!(type_satisfies(&Type::I64, &Type::I64));
        assert!(type_satisfies(&Type::Struct(0), &Type::HeapRef));
        assert!(type_satisfies(&Type::array(Type::F64), &Type::HeapRef));
        assert!(!type_satisfies(&Type::HeapRef, &Type::Struct(0)));
        assert!(!type_satisfies(&Type::HeapRef, &Type::array(Type::F64)));
        assert!(!type_satisfies(&Type::Struct(0), &Type::Struct(1)));
        assert!(!type_satisfies(
            &Type::array(Type::U64),
            &Type::array(Type::F64)
        ));
    }

    #[test]
    fn named_types_satisfy_only_themselves() {
        assert!(type_satisfies(&Type::Named(0), &Type::Named(0)));
        assert!(!type_satisfies(&Type::Named(0), &Type::Named(1)));
        assert!(!type_satisfies(&Type::Named(0), &Type::U64));
        assert!(!type_satisfies(&Type::U64, &Type::Named(0)));
        assert!(!type_satisfies(&Type::Named(0), &Type::HeapRef));
        assert!(type_satisfies(
            &Type::array(Type::Named(0)),
            &Type::array(Type::Named(0))
        ));
        assert!(!type_satisfies(
            &Type::array(Type::Named(0)),
            &Type::array(Type::U64)
        ));
    }

    #[test]
    fn unequal_types_join_to_a_sticky_conflict() {
        let i = TypeSlot::Known(Type::I64);
        let f = TypeSlot::Known(Type::F64);
        assert_eq!(i.join(&i), i);
        let conflict = i.join(&f);
        assert_eq!(conflict, TypeSlot::Conflict(Type::I64, Type::F64));
        assert_eq!(conflict.join(&i), conflict);
        assert_eq!(i.join(&conflict), conflict);
        assert_eq!(conflict.to_string(), "i64 | f64");
    }

    #[test]
    fn a_conflict_never_satisfies_expect() {
        let conflict = TypeSlot::Conflict(Type::I64, Type::I64);
        let err = conflict.expect(&Type::I64).unwrap_err();
        assert!(matches!(err.kind(), DiagnosticKind::TypeMismatch { .. }));
        assert!(
            TypeSlot::Known(Type::Struct(3))
                .expect(&Type::HeapRef)
                .is_ok()
        );
        let err = TypeSlot::Known(Type::U64).expect(&Type::I64).unwrap_err();
        assert_eq!(
            err.kind(),
            &DiagnosticKind::TypeMismatch {
                expected: "i64".into(),
                found: "u64".into()
            }
        );
    }
}
