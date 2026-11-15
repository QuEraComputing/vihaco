// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Abstract values: the [`Lattice`] and [`TypedSlot`] traits and the standard
//! [`TypeSlot`].

use std::fmt;

use vihaco_abi::Type;

use crate::diagnostic::{Expected, VerifyError};

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

    /// Record that `self`, the result of a [`join`](Lattice::join) that
    /// changed the saved value, was formed at the join point `address`.
    ///
    /// The standard frame calls this for every slot a join changes, so
    /// [`TypeSlot`] can remember where paths first disagreed and diagnostics
    /// can point there. The default keeps the value as it is.
    fn at_join(self, address: u32) -> Self {
        let _ = address;
        self
    }
}

/// Slots that can represent vihaco's [`Type`]s.
///
/// Standard rules bound only on this trait, so they work with any embedder's
/// slot type, including ones that track extra facts on top of a type.
pub trait TypedSlot: Lattice {
    /// The slot for a value known to have type `ty`.
    fn of_type(ty: Type) -> Self;

    /// The type every path agrees this slot has, or `None` (for example after
    /// a conflict). Rules that accept a family of types, such as any
    /// `array T`, inspect it.
    fn known_type(&self) -> Option<&Type>;

    /// This slot as a standard [`TypeSlot`], for diagnostics. Slots that track
    /// more than a type report the type part.
    fn to_type_slot(&self) -> TypeSlot;

    /// Accept this slot where a value of type `expected` is required, or
    /// report a [`DiagnosticKind::TypeMismatch`](crate::DiagnosticKind::TypeMismatch)
    /// (without a [role](VerifyError::with_role)).
    ///
    /// Only a slot whose [`known_type`](TypedSlot::known_type)
    /// [satisfies](type_satisfies) `expected` passes, so a conflict never
    /// does.
    fn check(&self, expected: &Type) -> Result<(), VerifyError> {
        match self.known_type() {
            Some(found) if type_satisfies(found, expected) => Ok(()),
            _ => Err(VerifyError::type_mismatch(expected, self.to_type_slot())),
        }
    }

    /// Accept this slot only if its type is exactly `expected`, not merely a
    /// type that satisfies it; for typed accesses to declared locals.
    fn check_exact(&self, expected: &Type) -> Result<(), VerifyError> {
        match self.known_type() {
            Some(found) if found == expected => Ok(()),
            _ => Err(VerifyError::type_mismatch(expected, self.to_type_slot())),
        }
    }
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
/// A conflict never satisfies [`TypedSlot::check`], so a value whose type
/// depends on the path taken cannot be used where a type is required; a
/// conflict that is never used is harmless.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeSlot {
    /// Every path agrees the value has this type.
    Known(Type),
    /// Paths disagreed at a join: the first two types seen.
    Conflict {
        /// The type saved at the join.
        first: Type,
        /// The type that arrived along another path.
        second: Type,
        /// The join point where the paths first disagreed, once the frame
        /// has recorded it (see [`Lattice::at_join`]).
        join: Option<u32>,
    },
}

impl TypeSlot {
    /// A conflict between `first` and `second`, not yet located at a join.
    pub fn conflict(first: Type, second: Type) -> Self {
        TypeSlot::Conflict {
            first,
            second,
            join: None,
        }
    }
}

impl Lattice for TypeSlot {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (TypeSlot::Known(a), TypeSlot::Known(b)) if a == b => self.clone(),
            (TypeSlot::Known(a), TypeSlot::Known(b)) => TypeSlot::conflict(a.clone(), b.clone()),
            (TypeSlot::Conflict { .. }, _) => self.clone(),
            (TypeSlot::Known(_), TypeSlot::Conflict { .. }) => other.clone(),
        }
    }

    fn at_join(self, address: u32) -> Self {
        match self {
            TypeSlot::Conflict {
                first,
                second,
                join: None,
            } => TypeSlot::Conflict {
                first,
                second,
                join: Some(address),
            },
            slot => slot,
        }
    }
}

impl TypedSlot for TypeSlot {
    fn of_type(ty: Type) -> Self {
        TypeSlot::Known(ty)
    }

    fn known_type(&self) -> Option<&Type> {
        match self {
            TypeSlot::Known(ty) => Some(ty),
            TypeSlot::Conflict { .. } => None,
        }
    }

    fn to_type_slot(&self) -> TypeSlot {
        self.clone()
    }
}

impl From<Type> for TypeSlot {
    fn from(ty: Type) -> Self {
        TypeSlot::Known(ty)
    }
}

/// A known slot shows its type; a conflict shows both, `u64 or f64`.
impl fmt::Display for TypeSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeSlot::Known(ty) => write!(f, "{ty}"),
            TypeSlot::Conflict { first, second, .. } => write!(f, "{first} or {second}"),
        }
    }
}

/// [`Expected`] renders as the type, `an array`, or `a value of a known type`.
impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expected::Type(ty) => write!(f, "{ty}"),
            Expected::AnyArray => f.write_str("an array"),
            Expected::AnyKnown => f.write_str("a value of a known type"),
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
    fn unequal_types_join_to_a_sticky_conflict_located_once() {
        let i = TypeSlot::Known(Type::I64);
        let f = TypeSlot::Known(Type::F64);
        assert_eq!(i.join(&i), i);
        let conflict = i.join(&f).at_join(5);
        assert_eq!(
            conflict,
            TypeSlot::Conflict {
                first: Type::I64,
                second: Type::F64,
                join: Some(5)
            }
        );
        // Later joins keep the first location.
        assert_eq!(conflict.join(&i).at_join(9), conflict);
        assert_eq!(i.join(&conflict), conflict);
        assert_eq!(conflict.to_string(), "i64 or f64");
    }

    #[test]
    fn a_conflict_never_satisfies_check() {
        let conflict = TypeSlot::conflict(Type::I64, Type::I64);
        let err = conflict.check(&Type::I64).unwrap_err();
        assert!(matches!(err.kind(), DiagnosticKind::TypeMismatch { .. }));
        assert!(
            TypeSlot::Known(Type::Struct(3))
                .check(&Type::HeapRef)
                .is_ok()
        );
        assert!(
            TypeSlot::Known(Type::Struct(3))
                .check_exact(&Type::HeapRef)
                .is_err()
        );
        let err = TypeSlot::Known(Type::U64).check(&Type::I64).unwrap_err();
        assert_eq!(
            err.kind(),
            &DiagnosticKind::TypeMismatch {
                role: None,
                expected: Expected::Type(Type::I64),
                found: TypeSlot::Known(Type::U64)
            }
        );
    }
}
