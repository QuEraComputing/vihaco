// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub mod dialect;

mod loadable;
mod runner;

pub use runner::resume_with_breaks;

use crate::{ExecutionState, RunState, debugger::dialect::debug};
use std::{collections::BTreeMap, convert::Infallible};
use vihaco_module::module::LocalModule;
use vihaco_runtime::{
    Apply, AutomaticMessage, Execute, HasProgram, Message, PrepareMessage, Reset, component,
};

/// Select continuous execution or a single step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugAction {
    Continue,
    Step,
}

enum PausedBreakpoint<K> {
    Explicit,
    Gutter { key: K },
}

/// A component that manages breakpoints.
///
/// This component can handle both explicit breakpoints (those appearing in
/// source) and "gutter" breakpoints, or those that are inserted (similar to
/// gdb's `INT3` insertion on `b`).
#[component]
#[instructions { debug }]
pub struct Debugger<K: Ord + 'static, I> {
    displaced: BTreeMap<K, I>,
    paused: Option<PausedBreakpoint<K>>,
}

impl<K: Ord + 'static, I> Default for Debugger<K, I> {
    fn default() -> Self {
        Self {
            displaced: BTreeMap::new(),
            paused: None,
        }
    }
}

impl<K: Ord + 'static, I> Reset for Debugger<K, I> {
    fn reset(&mut self) {
        self.paused = None;
    }
}

impl<K: Ord + 'static, I> Debugger<K, I> {
    /// Discard all saved breakpoint instructions and paused-breakpoint state.
    ///
    /// Use this when discarding the patched program, before using this debugger
    /// with a different image. This does not restore instructions in the old
    /// program. To remove breakpoints while keeping that program usable, call
    /// [`Self::clear_all`] instead. Invocation-only resets retain breakpoints.
    pub fn forget_breakpoints(&mut self) {
        self.displaced.clear();
        self.paused = None;
    }
}

pub enum IsBreakpointToggled {
    Toggled,
    Untoggled,
}

impl From<IsBreakpointInstalled> for IsBreakpointToggled {
    fn from(value: IsBreakpointInstalled) -> Self {
        match value {
            IsBreakpointInstalled::Installed(_) => Self::Toggled,
            IsBreakpointInstalled::No => Self::Untoggled,
        }
    }
}

impl<K: Ord + Clone + 'static, I: Clone> Debugger<K, I> {
    /// Installs a gutter breakpoint and retains the displaced instruction.
    pub fn set_breakpoint<P>(&mut self, program: &mut P, key: K) -> IsBreakpointToggled
    where
        P: BreakpointPatchableProgram<K, Instruction = I>,
    {
        if self.displaced.contains_key(&key) {
            return IsBreakpointToggled::Toggled;
        }
        let IsBreakpointInstalled::Installed(displaced) = program.install_breakpoint(&key) else {
            return IsBreakpointToggled::Untoggled;
        };
        self.displaced.insert(key, displaced);
        IsBreakpointToggled::Toggled
    }

    /// Restores a gutter breakpoint's displaced instruction.
    pub fn clear_breakpoint<P>(&mut self, program: &mut P, key: K) -> IsBreakpointToggled
    where
        P: BreakpointPatchableProgram<K, Instruction = I>,
    {
        let Some(displaced) = self.displaced.get(&key).cloned() else {
            return IsBreakpointToggled::Untoggled;
        };
        if program
            .uninstall_breakpoint(&key, displaced)
            .is_not_installed()
        {
            return IsBreakpointToggled::Toggled;
        }
        self.displaced.remove(&key);
        IsBreakpointToggled::Untoggled
    }

    pub fn toggle_breakpoint<P>(&mut self, program: &mut P, key: K) -> IsBreakpointToggled
    where
        P: BreakpointPatchableProgram<K, Instruction = I>,
    {
        if self.displaced.contains_key(&key) {
            self.clear_breakpoint(program, key)
        } else {
            self.set_breakpoint(program, key)
        }
    }

    pub fn clear_all<P>(&mut self, program: &mut P)
    where
        P: BreakpointPatchableProgram<K, Instruction = I>,
    {
        let keys: Vec<_> = self.displaced.keys().cloned().collect();
        for key in keys {
            self.clear_breakpoint(program, key);
        }
    }

    pub fn breakpoints(&self) -> impl ExactSizeIterator<Item = &K> + '_ {
        self.displaced.keys()
    }

    pub fn displaced(&self, key: &K) -> Option<&I> {
        self.displaced.get(key)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.displaced.contains_key(key)
    }
}

/// An effect that propagates a hit breakpoint.
pub struct Break;

/// Identify the breakpoint being executed.
#[derive(Debug)]
pub struct BreakpointMessage<K = u32> {
    pub key: K,
}

impl<K: 'static> Message for BreakpointMessage<K> {}

impl<K> AutomaticMessage for BreakpointMessage<K> {}

impl<P, F, Context: ?Sized> PrepareMessage<P, F, debug::Breakpoint, Context> for BreakpointMessage
where
    P: HasProgram<Program = ExecutionState<DebuggerState>>,
{
    fn prepare(parent: &mut P, _: &debug::Breakpoint, _: &Context) -> eyre::Result<Self> {
        let program = parent.program();
        eyre::ensure!(
            matches!(program.run_state(), RunState::Runnable),
            "program is not running"
        );
        let key = program
            .pc()
            .checked_sub(1)
            .ok_or_else(|| eyre::eyre!("breakpoint has no instruction position"))?;
        Ok(Self { key })
    }
}

/// Debugger-specific execution suspension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebuggerState {
    Breakpoint { resume_pc: u32 },
}

impl Apply<Break> for ExecutionState<DebuggerState> {
    fn apply(&mut self, _: Break) {
        self.suspend(DebuggerState::Breakpoint {
            resume_pc: self.pc(),
        });
    }
}

/// This instruction set has a breakpoint instruction. Implement this for your
/// component's runtime instruction set to define how it can build a breakpoint
/// instruction.
pub trait HasBreakpointInstruction: Sized + Clone {
    fn new_breakpoint() -> Self;
}

pub enum IsBreakpointInstalled<I = ()> {
    Installed(I),
    No,
}

impl<I> IsBreakpointInstalled<I> {
    pub fn is_installed(&self) -> bool {
        matches!(self, IsBreakpointInstalled::Installed(_))
    }

    pub fn is_not_installed(&self) -> bool {
        !self.is_installed()
    }
}

pub trait BreakpointPatchableProgram<K> {
    type Instruction: HasBreakpointInstruction;

    fn install_breakpoint(&mut self, index: &K) -> IsBreakpointInstalled<Self::Instruction>;
    /// Restore the instruction displaced by a breakpoint.
    fn uninstall_breakpoint(
        &mut self,
        index: &K,
        displaced: Self::Instruction,
    ) -> IsBreakpointInstalled;
}

impl<I, V, Ty, Info> BreakpointPatchableProgram<u32> for LocalModule<I, V, Ty, Info>
where
    I: HasBreakpointInstruction,
{
    type Instruction = I;

    fn install_breakpoint(&mut self, index: &u32) -> IsBreakpointInstalled<I> {
        let Some(slot) = self.code.get_mut(*index as usize) else {
            return IsBreakpointInstalled::No;
        };
        IsBreakpointInstalled::Installed(std::mem::replace(slot, I::new_breakpoint()))
    }

    fn uninstall_breakpoint(&mut self, index: &u32, displaced: I) -> IsBreakpointInstalled {
        let Some(slot) = self.code.get_mut(*index as usize) else {
            return IsBreakpointInstalled::No;
        };
        *slot = displaced;
        IsBreakpointInstalled::Installed(())
    }
}

impl<K: Ord + 'static, I> Execute<debug::Breakpoint> for Debugger<K, I> {
    type Message = BreakpointMessage<K>;
    type Effect = Break;
    type Error = Infallible;

    fn execute(
        &mut self,
        _instruction: &debug::Breakpoint,
        message: Self::Message,
    ) -> Result<Self::Effect, Self::Error> {
        self.paused = Some(if self.displaced.contains_key(&message.key) {
            PausedBreakpoint::Gutter { key: message.key }
        } else {
            PausedBreakpoint::Explicit
        });
        Ok(Break)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Instruction {
        Value(u64),
        Breakpoint,
    }

    impl HasBreakpointInstruction for Instruction {
        fn new_breakpoint() -> Self {
            Self::Breakpoint
        }
    }

    #[test]
    fn forgetting_breakpoints_cannot_restore_old_instructions_into_a_new_program() {
        let mut debugger = Debugger::<u32, Instruction>::default();
        let mut original = LocalModule::<Instruction, (), ()> {
            code: vec![Instruction::Value(11)],
            ..Default::default()
        };
        debugger.set_breakpoint(&mut original, 0);
        debugger
            .execute(&debug::Breakpoint, BreakpointMessage { key: 0 })
            .unwrap();

        debugger.forget_breakpoints();
        assert_eq!(debugger.breakpoints().len(), 0);
        assert!(debugger.paused.is_none());

        let mut replacement = LocalModule::<Instruction, (), ()> {
            code: vec![Instruction::Value(42)],
            ..Default::default()
        };
        assert!(matches!(
            debugger.clear_breakpoint(&mut replacement, 0),
            IsBreakpointToggled::Untoggled
        ));
        assert_eq!(replacement.code, vec![Instruction::Value(42)]);
        assert_eq!(original.code, vec![Instruction::Breakpoint]);

        debugger.set_breakpoint(&mut replacement, 0);
        debugger.clear_all(&mut replacement);
        assert_eq!(replacement.code, vec![Instruction::Value(42)]);
    }

    #[test]
    fn invocation_reset_retains_saved_breakpoint_instructions() {
        let mut debugger = Debugger::<u32, Instruction>::default();
        let mut program = LocalModule::<Instruction, (), ()> {
            code: vec![Instruction::Value(11)],
            ..Default::default()
        };
        debugger.set_breakpoint(&mut program, 0);
        debugger
            .execute(&debug::Breakpoint, BreakpointMessage { key: 0 })
            .unwrap();

        debugger.reset();
        assert!(debugger.paused.is_none());
        assert!(debugger.contains(&0));
        debugger.clear_all(&mut program);
        assert_eq!(program.code, vec![Instruction::Value(11)]);
    }
}
