// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Instruction-owned typing rules for load-time verification.
//!
//! Verification mirrors capability-based lowering ([`crate::Lower`]): each
//! component implements [`Verify<C>`] for its runtime instructions, stating the
//! verifier capabilities it needs as bounds on `C`. A composite's generated
//! instruction aggregate implements `Verify<C>` exactly when every instruction
//! it contains does, so Rust reports any instruction without a rule.
//!
//! A verification driver first uses [`Verify::flow`], which is stateless, to
//! find basic blocks and reachable code; it then calls [`Verify::verify`] on
//! each instruction to check and update abstract state. See
//! `design/verifier.md`.

use smallvec::SmallVec;

/// The typing rule for one instruction.
///
/// Instruction authors express the capabilities they need (operand types,
/// local types, signatures, struct layouts, component-specific state) as
/// bounds on `C`.
pub trait Verify<C: ?Sized> {
    /// Where control goes after this instruction.
    ///
    /// This must depend only on the instruction's operands, never on abstract
    /// state, so that block boundaries and reachability do not depend on
    /// whether any rule succeeds. Most instructions continue to the next
    /// address; control-flow instructions override this.
    fn flow(&self) -> Flow {
        Flow::Next
    }

    /// Check this instruction's inputs against the abstract state in
    /// `context` and update the state to the instruction's post-state.
    fn verify(&self, context: &mut C) -> eyre::Result<()>;
}

/// Where control goes after an instruction, as reported by [`Verify::flow`].
///
/// Addresses are section-local instruction addresses, as in branch operands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Continue at the next instruction. Calls continue here too: callees are
    /// verified separately.
    Next,
    /// Continue at each listed address, with no fallthrough; for example an
    /// unconditional branch has one target and a conditional branch two.
    Jump(SmallVec<[u32; 2]>),
    /// Leave the current function.
    Return,
    /// Stop the machine.
    Halt,
}

impl Flow {
    /// Continue at exactly one address.
    pub fn jump(target: u32) -> Self {
        Flow::Jump(SmallVec::from_buf_and_len([target, 0], 1))
    }

    /// Continue at any of the given addresses.
    pub fn jump_to(targets: impl IntoIterator<Item = u32>) -> Self {
        Flow::Jump(targets.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Depth(usize);

    struct Push;
    struct Pop;
    struct BranchIf(u32, u32);

    impl Verify<Depth> for Push {
        fn verify(&self, depth: &mut Depth) -> eyre::Result<()> {
            depth.0 += 1;
            Ok(())
        }
    }

    impl Verify<Depth> for Pop {
        fn verify(&self, depth: &mut Depth) -> eyre::Result<()> {
            depth.0 = depth
                .0
                .checked_sub(1)
                .ok_or_else(|| eyre::eyre!("underflow"))?;
            Ok(())
        }
    }

    impl Verify<Depth> for BranchIf {
        fn flow(&self) -> Flow {
            Flow::jump_to([self.0, self.1])
        }

        fn verify(&self, depth: &mut Depth) -> eyre::Result<()> {
            Pop.verify(depth)
        }
    }

    #[test]
    fn rules_update_state_and_control_flow_is_stateless() {
        let mut depth = Depth(0);
        Push.verify(&mut depth).unwrap();
        assert_eq!(depth.0, 1);
        assert_eq!(<Push as Verify<Depth>>::flow(&Push), Flow::Next);

        let branch = BranchIf(4, 9);
        // Flow is known without running the rule, even when the rule would fail.
        assert_eq!(
            <BranchIf as Verify<Depth>>::flow(&branch),
            Flow::Jump(SmallVec::from_slice(&[4, 9]))
        );
        branch.verify(&mut depth).unwrap();
        assert!(branch.verify(&mut depth).is_err());
    }

    #[test]
    fn single_target_jumps_hold_one_address() {
        let Flow::Jump(targets) = Flow::jump(7) else {
            panic!("expected a jump");
        };
        assert_eq!(targets.as_slice(), &[7]);
    }
}
