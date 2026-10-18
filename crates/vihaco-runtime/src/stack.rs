// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Framework stack preparation and effect-application protocols.

use crate::{BuildMessage, Composite, Execute, Field, Message, PopStack, PushStack, StackValue};

/// Copied operands prepared for an instruction, distinct from its payload.
///
/// Tuple builders support arities 0–16. Fields retain their declared order,
/// while extraction proceeds right to left: `(lhs, rhs)` pops `rhs` first.
/// Successful pops remain consumed if a later pop or CPU execution fails.
/// The empty tuple consumes nothing and does not access the stack capability.
/// Construction itself requires no allocation or stack borrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopFromStack<Values>(pub Values);

impl<Values: 'static> Message for PopFromStack<Values> {}

/// Results to insert during the framework's effect-application phase.
///
/// [`Apply`] supports tuples of arity 0–16 (including heterogeneous items) and
/// homogeneous arrays of any length. Values are inserted left to right, so the
/// last value becomes the top. Empty tuples/arrays insert nothing. The wrapper
/// owns values, not storage references, and does not apply itself automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PushToStack<Values>(pub Values);

/// Framework wiring from composite instruction preparation to operand storage.
///
/// The completed program-owned route supplies this connection through composite
/// generation; ordinary component/composite authors only select `PopFromStack`
/// and `PushToStack` types. Boundary A checks capability constraints through
/// compilation; runtime coverage will use ProgramImage when that wiring lands.
///
/// Opting into this capability selects the global tuple [`BuildMessage`]
/// implementations for compatible child message types. A field name or merely
/// containing a ProgramImage does not establish the capability. Custom
/// preparation can use a distinct message type with its own builder.
///
/// No canonical-Boolean validation policy is needed: [`StackValue`] Boolean
/// extraction interprets every nonzero word as true.
pub trait HasOperandStack<I> {
    /// The checked operand source used for this instruction.
    type Source;

    /// Borrow the operand source for a tuple of the supplied static arity.
    ///
    /// The arity allows a later cached implementation to prepare storage once.
    /// This accessor must not consume operands, bulk-precheck their availability,
    /// or otherwise change the sequential partial-consumption semantics.
    /// Empty tuple builders do not invoke it.
    fn operand_stack(&mut self, arity: usize) -> &mut Self::Source;
}

/// Framework hook to apply one effect during [`crate::Step`].
///
/// Ordinary authors select `PushToStack` effects; the completed program-owned
/// route supplies the Step wiring that invokes this hook. Implementations are
/// extension plumbing for storage/effects, not required component boilerplate.
///
/// Stack output implementations use direct typed insertion. This trait neither
/// executes instructions nor supplies a Step or observer callbacks. The caller
/// handles [`crate::Effects`] in order, applying each effect after execution and
/// before its applied/finished observation phases. Applied values are not
/// rolled back if later work fails. Custom fallible effect handling remains in
/// Step; stack insertion follows [`PushStack`]'s ordinary
/// allocation behavior.
pub trait Apply<E> {
    /// Apply a single effect to this destination.
    fn apply(&mut self, effect: E);
}

impl<C, F, I> BuildMessage<F, I, PopFromStack<()>> for C
where
    C: Composite + Field<F> + HasOperandStack<I>,
    C::Child: Execute<I, Message = PopFromStack<()>>,
{
    #[inline]
    fn build_message(&mut self, _: &I) -> eyre::Result<PopFromStack<()>> {
        Ok(PopFromStack(()))
    }
}

impl<D> Apply<PushToStack<()>> for D {
    #[inline]
    fn apply(&mut self, _: PushToStack<()>) {}
}

impl<D, T: StackValue, const N: usize> Apply<PushToStack<[T; N]>> for D
where
    D: PushStack<T::Storage>,
{
    #[inline]
    fn apply(&mut self, effect: PushToStack<[T; N]>) {
        for value in effect.0 {
            value.push_to(self);
        }
    }
}

// Expand once here into ordinary global per-arity impls, never per instruction
// or composite. The explicit message parameter keeps these disjoint from the
// NoMessage blanket and consumers' distinct custom-message implementations.
macro_rules! stack_tuple {
    (@pop $source:ident;) => {};
    (@pop $source:ident; $ty:ident:$value:ident $(, $rest:ident:$other:ident)*) => {
        stack_tuple!(@pop $source; $($rest:$other),*);
        let $value = $ty::pop_from($source)?;
    };
    ($($ty:ident:$value:ident),+) => {
        impl<Parent, Route, Inst, $($ty: StackValue),+>
            BuildMessage<Route, Inst, PopFromStack<($($ty,)+)>> for Parent
        where
            Parent: Composite + Field<Route> + HasOperandStack<Inst>,
            Parent::Child: Execute<Inst, Message = PopFromStack<($($ty,)+)>>,
            Parent::Source: $(PopStack<$ty::Storage> +)+ Sized,
        {
            #[inline]
            fn build_message(&mut self, _: &Inst) -> eyre::Result<PopFromStack<($($ty,)+)>> {
                let source = self.operand_stack([$(stringify!($ty)),+].len());
                stack_tuple!(@pop source; $($ty:$value),+);
                Ok(PopFromStack(($($value,)+)))
            }
        }

        impl<Destination, $($ty: StackValue),+>
            Apply<PushToStack<($($ty,)+)>> for Destination
        where
            Destination: $(PushStack<$ty::Storage> +)+ Sized,
        {
            #[inline]
            fn apply(&mut self, effect: PushToStack<($($ty,)+)>) {
                let ($($value,)+) = effect.0;
                $($value.push_to(self);)+
            }
        }
    };
}

stack_tuple!(A:a);
stack_tuple!(A:a, B:b);
stack_tuple!(A:a, B:b, C:c);
stack_tuple!(A:a, B:b, C:c, D:d);
stack_tuple!(A:a, B:b, C:c, D:d, E:e);
stack_tuple!(A:a, B:b, C:c, D:d, E:e, F:f);
stack_tuple!(A:a, B:b, C:c, D:d, E:e, F:f, G:g);
stack_tuple!(A:a, B:b, C:c, D:d, E:e, F:f, G:g, H:h);
stack_tuple!(A:a, B:b, C:c, D:d, E:e, F:f, G:g, H:h, I:i);
stack_tuple!(A:a, B:b, C:c, D:d, E:e, F:f, G:g, H:h, I:i, J:j);
stack_tuple!(A:a, B:b, C:c, D:d, E:e, F:f, G:g, H:h, I:i, J:j, K:k);
stack_tuple!(A:a, B:b, C:c, D:d, E:e, F:f, G:g, H:h, I:i, J:j, K:k, L:l);
stack_tuple!(A:a, B:b, C:c, D:d, E:e, F:f, G:g, H:h, I:i, J:j, K:k, L:l, M:m);
stack_tuple!(A:a, B:b, C:c, D:d, E:e, F:f, G:g, H:h, I:i, J:j, K:k, L:l, M:m, N:n);
stack_tuple!(A:a, B:b, C:c, D:d, E:e, F:f, G:g, H:h, I:i, J:j, K:k, L:l, M:m, N:n, O:o);
stack_tuple!(A:a, B:b, C:c, D:d, E:e, F:f, G:g, H:h, I:i, J:j, K:k, L:l, M:m, N:n, O:o, P:p);
