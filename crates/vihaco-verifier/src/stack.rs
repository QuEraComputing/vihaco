// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The [`OperandStack`] capability and the standard abstract frame.

use std::fmt;

use vihaco_abi::Type;

use crate::capability::Locals;
use crate::diagnostic::VerifyError;
use crate::slot::{Lattice, TypedSlot};
use crate::state::AbstractState;

/// Capability: the abstract operand stack of the function being verified.
///
/// Depth counts operands only, from the function's first operand, so popping
/// an empty stack is a [`StackUnderflow`]: no operand instruction can reach
/// the function's locals or its caller's frame.
///
/// [`StackUnderflow`]: crate::DiagnosticKind::StackUnderflow
pub trait OperandStack {
    /// The abstract value held in each stack slot.
    type Slot: Lattice;

    /// Pop the top operand.
    fn pop(&mut self) -> Result<Self::Slot, VerifyError>;

    /// Pop the top operand, requiring it to satisfy `expected`.
    fn pop_expect(&mut self, expected: &Type) -> Result<Self::Slot, VerifyError>
    where
        Self::Slot: TypedSlot,
    {
        let slot = self.pop()?;
        slot.expect(expected)?;
        Ok(slot)
    }

    /// Pop the top `n` operands, returned bottom first.
    ///
    /// Bounded: if fewer than `n` operands are present this reports a single
    /// [`StackUnderflow`] and leaves the stack unchanged, so a huge count
    /// costs at most `O(depth)`.
    ///
    /// [`StackUnderflow`]: crate::DiagnosticKind::StackUnderflow
    fn pop_n(&mut self, n: usize) -> Result<Vec<Self::Slot>, VerifyError> {
        if n > self.depth() {
            return Err(VerifyError::stack_underflow());
        }
        let mut popped = Vec::with_capacity(n);
        for _ in 0..n {
            popped.push(self.pop()?);
        }
        popped.reverse();
        Ok(popped)
    }

    /// Push an operand.
    fn push(&mut self, slot: Self::Slot);

    /// The top operand, without popping it.
    fn peek(&self) -> Result<&Self::Slot, VerifyError>;

    /// The number of operands on the stack.
    fn depth(&self) -> usize;
}

/// Apply a fixed stack effect: pop `inputs` (listed bottom first, so the last
/// input is the top of the stack), checking each against its type, then push
/// `outputs` in order.
///
/// Underflow is detected before anything is popped, so it is reported once
/// regardless of how many inputs are missing.
///
/// ```
/// # use vihaco_verifier::{Frame, FrameState, OperandStack, TypeSlot, stack_effect};
/// # use vihaco_abi::Type;
/// let mut cx = FrameState::new(Frame::<TypeSlot>::with_operands(
///     vec![],
///     vec![Type::I64.into(), Type::I64.into()],
/// ));
/// stack_effect(&mut cx, [Type::I64, Type::I64], [Type::Bool]).unwrap();
/// assert_eq!(cx.peek().unwrap(), &TypeSlot::Known(Type::Bool));
/// ```
pub fn stack_effect<C, const N: usize, const M: usize>(
    cx: &mut C,
    inputs: [Type; N],
    outputs: [Type; M],
) -> eyre::Result<()>
where
    C: OperandStack + ?Sized,
    C::Slot: TypedSlot,
{
    if cx.depth() < N {
        return Err(VerifyError::stack_underflow().into());
    }
    for expected in inputs.iter().rev() {
        cx.pop_expect(expected)?;
    }
    for ty in outputs {
        cx.push(C::Slot::of_type(ty));
    }
    Ok(())
}

/// One function's abstract frame at a program point: a fixed-length list of
/// local slots and a variable-length operand stack.
///
/// This is the standard [`AbstractState::Snapshot`] of [`FrameState`], the
/// state the engine saves at each leader and exports in type maps.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Frame<S> {
    locals: Vec<S>,
    operands: Vec<S>,
}

impl<S: Lattice> Frame<S> {
    /// A frame with the given locals and an empty operand stack: the entry
    /// state of a function.
    pub fn new(locals: Vec<S>) -> Self {
        Self::with_operands(locals, Vec::new())
    }

    /// A frame with the given locals and operands (bottom first).
    pub fn with_operands(locals: Vec<S>, operands: Vec<S>) -> Self {
        Self { locals, operands }
    }

    /// The local slots, local 0 first.
    pub fn locals(&self) -> &[S] {
        &self.locals
    }

    /// The operand slots, bottom first.
    pub fn operands(&self) -> &[S] {
        &self.operands
    }

    /// The operand depth.
    pub fn depth(&self) -> usize {
        self.operands.len()
    }

    /// Join `incoming` into `self`, the frame saved at a join point.
    ///
    /// Operand depths must agree ([`StackDepthMismatch`] otherwise), and so
    /// must local counts ([`LocalCountMismatch`]); then locals and operands
    /// are joined slot-wise. Returns whether `self` strictly changed. On error
    /// `self` is left unchanged.
    ///
    /// [`StackDepthMismatch`]: crate::DiagnosticKind::StackDepthMismatch
    /// [`LocalCountMismatch`]: crate::DiagnosticKind::LocalCountMismatch
    pub fn join(&mut self, incoming: &Self) -> Result<bool, VerifyError> {
        if self.operands.len() != incoming.operands.len() {
            return Err(VerifyError::stack_depth_mismatch(
                self.operands.len(),
                incoming.operands.len(),
            ));
        }
        if self.locals.len() != incoming.locals.len() {
            return Err(VerifyError::new(
                crate::DiagnosticKind::LocalCountMismatch {
                    expected: self.locals.len(),
                    found: incoming.locals.len(),
                },
            ));
        }
        let mut changed = false;
        let saved = self.locals.iter_mut().chain(self.operands.iter_mut());
        let arriving = incoming.locals.iter().chain(incoming.operands.iter());
        for (saved, arriving) in saved.zip(arriving) {
            let joined = saved.join(arriving);
            if joined != *saved {
                *saved = joined;
                changed = true;
            }
        }
        Ok(changed)
    }
}

impl<S: fmt::Display> fmt::Display for Frame<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn list<S: fmt::Display>(f: &mut fmt::Formatter<'_>, slots: &[S]) -> fmt::Result {
            for (i, slot) in slots.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{slot}")?;
            }
            Ok(())
        }
        f.write_str("locals [")?;
        list(f, &self.locals)?;
        f.write_str("] operands [")?;
        list(f, &self.operands)?;
        f.write_str("]")
    }
}

/// The standard verifier context for one function: a [`Frame`] plus the
/// current position.
///
/// Implements [`OperandStack`], [`Locals`] and [`AbstractState`]. A machine-specific
/// context can wrap a `FrameState`, delegate [`OperandStack`] to it, and join
/// its own state alongside the frame's.
#[derive(Clone, Debug)]
pub struct FrameState<S> {
    frame: Frame<S>,
    function: u32,
    address: u32,
}

impl<S: Lattice> FrameState<S> {
    /// A context starting from `frame`.
    pub fn new(frame: Frame<S>) -> Self {
        Self {
            frame,
            function: 0,
            address: 0,
        }
    }

    /// The current frame.
    pub fn frame(&self) -> &Frame<S> {
        &self.frame
    }

    /// The function of the instruction being verified.
    pub fn function(&self) -> u32 {
        self.function
    }

    /// The address of the instruction being verified.
    pub fn address(&self) -> u32 {
        self.address
    }
}

/// Bounds-checked against the frame's locals.
impl<S: Lattice> Locals for FrameState<S> {
    fn local_count(&self) -> u32 {
        // Frames come from declared locals, capped well below `u32::MAX`.
        u32::try_from(self.frame.locals.len()).unwrap_or(u32::MAX)
    }

    fn local(&self, index: u32) -> Result<&S, VerifyError> {
        let count = self.local_count();
        self.frame
            .locals
            .get(index as usize)
            .ok_or_else(|| VerifyError::local_index_out_of_range(index, count))
    }

    fn set_local(&mut self, index: u32, slot: S) -> Result<(), VerifyError> {
        let count = self.local_count();
        let local = self
            .frame
            .locals
            .get_mut(index as usize)
            .ok_or_else(|| VerifyError::local_index_out_of_range(index, count))?;
        *local = slot;
        Ok(())
    }
}

impl<S: Lattice> OperandStack for FrameState<S> {
    type Slot = S;

    fn pop(&mut self) -> Result<S, VerifyError> {
        self.frame
            .operands
            .pop()
            .ok_or_else(VerifyError::stack_underflow)
    }

    fn pop_n(&mut self, n: usize) -> Result<Vec<S>, VerifyError> {
        let depth = self.frame.operands.len();
        let base = depth
            .checked_sub(n)
            .ok_or_else(VerifyError::stack_underflow)?;
        Ok(self.frame.operands.split_off(base))
    }

    fn push(&mut self, slot: S) {
        self.frame.operands.push(slot);
    }

    fn peek(&self) -> Result<&S, VerifyError> {
        self.frame
            .operands
            .last()
            .ok_or_else(VerifyError::stack_underflow)
    }

    fn depth(&self) -> usize {
        self.frame.operands.len()
    }
}

impl<S: Lattice> AbstractState for FrameState<S> {
    type Snapshot = Frame<S>;

    fn snapshot(&self) -> Frame<S> {
        self.frame.clone()
    }

    fn restore(&mut self, snapshot: &Frame<S>) {
        self.frame.clone_from(snapshot);
    }

    fn join(saved: &mut Frame<S>, incoming: &Frame<S>) -> Result<bool, VerifyError> {
        saved.join(incoming)
    }

    fn set_address(&mut self, function: u32, address: u32) {
        self.function = function;
        self.address = address;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiagnosticKind, TypeSlot};

    fn slot(ty: Type) -> TypeSlot {
        TypeSlot::Known(ty)
    }

    #[test]
    fn pop_n_is_bounded_and_ordered() {
        let mut cx = FrameState::new(Frame::with_operands(
            vec![],
            vec![slot(Type::I64), slot(Type::F64)],
        ));
        let err = cx.pop_n(usize::MAX).unwrap_err();
        assert_eq!(err.kind(), &DiagnosticKind::StackUnderflow);
        assert_eq!(cx.depth(), 2, "an underflowing pop_n leaves the stack");
        assert_eq!(cx.pop_n(2).unwrap(), vec![slot(Type::I64), slot(Type::F64)]);
        assert_eq!(cx.depth(), 0);
        assert!(cx.peek().is_err());
    }

    #[test]
    fn the_default_pop_n_matches() {
        struct Depth(Vec<TypeSlot>);
        impl OperandStack for Depth {
            type Slot = TypeSlot;
            fn pop(&mut self) -> Result<TypeSlot, VerifyError> {
                self.0.pop().ok_or_else(VerifyError::stack_underflow)
            }
            fn push(&mut self, slot: TypeSlot) {
                self.0.push(slot);
            }
            fn peek(&self) -> Result<&TypeSlot, VerifyError> {
                self.0.last().ok_or_else(VerifyError::stack_underflow)
            }
            fn depth(&self) -> usize {
                self.0.len()
            }
        }
        let mut cx = Depth(vec![slot(Type::I64), slot(Type::F64)]);
        assert!(cx.pop_n(3).is_err());
        assert_eq!(cx.pop_n(2).unwrap(), vec![slot(Type::I64), slot(Type::F64)]);
    }

    #[test]
    fn stack_effects_check_types_top_last() {
        let mut cx = FrameState::new(Frame::with_operands(
            vec![],
            vec![slot(Type::I64), slot(Type::F64)],
        ));
        let err = stack_effect(&mut cx, [Type::F64, Type::I64], [])
            .unwrap_err()
            .downcast::<VerifyError>()
            .unwrap();
        assert_eq!(err, VerifyError::type_mismatch("i64", "f64"));
    }

    #[test]
    fn frames_join_slot_wise_and_reject_depth_changes() {
        let mut saved = Frame::with_operands(vec![slot(Type::I64)], vec![slot(Type::U64)]);
        let same = saved.clone();
        assert!(!saved.join(&same).unwrap());
        let other = Frame::with_operands(vec![slot(Type::F64)], vec![slot(Type::U64)]);
        assert!(saved.join(&other).unwrap());
        assert_eq!(saved.locals()[0], TypeSlot::Conflict(Type::I64, Type::F64));
        let deeper = Frame::with_operands(vec![slot(Type::I64)], vec![slot(Type::U64); 2]);
        let before = saved.clone();
        assert_eq!(
            saved.join(&deeper).unwrap_err(),
            VerifyError::stack_depth_mismatch(1, 2)
        );
        assert_eq!(saved, before);
        assert_eq!(saved.to_string(), "locals [i64 | f64] operands [u64]");
    }

    #[test]
    fn locals_are_bounds_checked() {
        let mut cx = FrameState::new(Frame::new(vec![slot(Type::I64)]));
        assert_eq!(cx.local(0).unwrap(), &slot(Type::I64));
        cx.set_local(0, slot(Type::Bool)).unwrap();
        assert_eq!(cx.local(0).unwrap(), &slot(Type::Bool));
        assert_eq!(
            cx.set_local(4, slot(Type::Bool)).unwrap_err().kind(),
            &DiagnosticKind::LocalIndexOutOfRange { index: 4, count: 1 }
        );
    }
}
