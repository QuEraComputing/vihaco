// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Component state from independent crates, combined in one machine.
//!
//! `sequencer` and `timing` are written as two separate component crates
//! would be: each defines its own state, capability, and instructions, and
//! implements its capability for any `WithState` whose state contains its own
//! (`ComponentStateOf`), never for a concrete combination. The `machine`
//! module combines both with `component_state!` and writes no delegation.

use vihaco_module::module::{FunctionInfo, LocalModule, Signature};
use vihaco_verifier::{
    DiagnosticKind, Flow, Type, Value, Verified, Verify, VerifyOptions, WithState,
};

/// A sequencer component crate: channels must be opened before playing.
mod sequencer {
    use std::collections::BTreeSet;
    use vihaco_verifier::{ComponentState, ComponentStateOf, Verify, VerifyError, WithState};

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Open(pub BTreeSet<u32>);

    impl ComponentState for Open {
        /// Open after a join only if open on every path.
        fn join(&mut self, incoming: &Self) -> Result<bool, VerifyError> {
            let before = self.0.len();
            self.0.retain(|channel| incoming.0.contains(channel));
            Ok(self.0.len() != before)
        }

        /// Nothing is assumed open after a call, as at a function entry.
        fn after_call(&mut self, _callee: Option<u32>, entry: &Self) {
            self.clone_from(entry);
        }
    }

    pub trait Channels {
        fn open(&mut self, channel: u32);
        fn require_open(&self, channel: u32) -> Result<(), VerifyError>;
    }

    impl<S: ComponentStateOf<Open>> Channels for WithState<'_, S> {
        fn open(&mut self, channel: u32) {
            self.component_mut::<Open>().0.insert(channel);
        }

        fn require_open(&self, channel: u32) -> Result<(), VerifyError> {
            if self.component::<Open>().0.contains(&channel) {
                Ok(())
            } else {
                Err(VerifyError::rule(format!("channel {channel} is not open")))
            }
        }
    }

    #[derive(Debug)]
    pub enum Seq {
        Open(u32),
        Play(u32),
    }

    impl<C: Channels + ?Sized> Verify<C> for Seq {
        fn verify(&self, cx: &mut C) -> eyre::Result<()> {
            match self {
                Seq::Open(channel) => cx.open(*channel),
                Seq::Play(channel) => cx.require_open(*channel)?,
            }
            Ok(())
        }
    }
}

/// A timing component crate: a budget of elapsed cycles, widened at joins.
mod timing {
    use vihaco_verifier::{ComponentState, ComponentStateOf, Verify, VerifyError, WithState};

    /// Cycles elapsed so far; `None` once paths disagree (widened).
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Elapsed(pub Option<u32>);

    impl ComponentState for Elapsed {
        fn join(&mut self, incoming: &Self) -> Result<bool, VerifyError> {
            if self.0.is_some() && self.0 != incoming.0 {
                self.0 = None;
                return Ok(true);
            }
            Ok(false)
        }

        /// A callee may wait any amount, so the elapsed time is unknown.
        fn after_call(&mut self, _callee: Option<u32>, _entry: &Self) {
            self.0 = None;
        }
    }

    pub trait Timing {
        fn wait(&mut self, cycles: u32);
        fn require_within(&self, budget: u32) -> Result<(), VerifyError>;
    }

    impl<S: ComponentStateOf<Elapsed>> Timing for WithState<'_, S> {
        fn wait(&mut self, cycles: u32) {
            let elapsed = self.component_mut::<Elapsed>();
            elapsed.0 = elapsed.0.and_then(|total| total.checked_add(cycles));
        }

        fn require_within(&self, budget: u32) -> Result<(), VerifyError> {
            match self.component::<Elapsed>().0 {
                Some(total) if total <= budget => Ok(()),
                Some(total) => Err(VerifyError::rule(format!(
                    "{total} cycles have elapsed, over the budget of {budget}"
                ))),
                None => Err(VerifyError::rule("elapsed time depends on the path taken")),
            }
        }
    }

    #[derive(Debug)]
    pub enum Clock {
        Wait(u32),
        Deadline(u32),
    }

    impl<C: Timing + ?Sized> Verify<C> for Clock {
        fn verify(&self, cx: &mut C) -> eyre::Result<()> {
            match self {
                Clock::Wait(cycles) => cx.wait(*cycles),
                Clock::Deadline(budget) => cx.require_within(*budget)?,
            }
            Ok(())
        }
    }
}

/// The machine crate: combines both states, writes no delegation.
mod machine {
    use super::sequencer::{Open, Seq};
    use super::timing::{Clock, Elapsed};
    use vihaco_verifier::{Flow, Verify, component_state};

    component_state! {
        #[derive(Clone, Debug, Default)]
        pub struct MachineState {
            pub open: Open,
            pub elapsed: Elapsed,
        }
    }

    /// The machine's instruction set, as a composite's aggregate would be.
    #[derive(Debug)]
    pub enum Inst {
        Seq(Seq),
        Clock(Clock),
        Halt,
    }

    impl<C: ?Sized> Verify<C> for Inst
    where
        Seq: Verify<C>,
        Clock: Verify<C>,
    {
        fn flow(&self) -> Flow {
            match self {
                Inst::Halt => Flow::Halt,
                _ => Flow::Next,
            }
        }

        fn verify(&self, cx: &mut C) -> eyre::Result<()> {
            match self {
                Inst::Seq(seq) => seq.verify(cx),
                Inst::Clock(clock) => clock.verify(cx),
                Inst::Halt => Ok(()),
            }
        }
    }
}

use machine::{Inst, MachineState};
use sequencer::{Open, Seq};
use timing::{Clock, Elapsed};

fn module(code: Vec<Inst>) -> LocalModule<Inst, Value, Type> {
    let main = FunctionInfo {
        name: 0,
        signature: Signature {
            params: vec![],
            ret: vec![],
        },
        locals: vec![],
        start_address: 0,
        end_address: code.len() as u32,
        file: 0,
    };
    LocalModule {
        code,
        functions: vec![main],
        strings: vec!["main".into()],
        main_function: Some(0),
        ..LocalModule::default()
    }
}

fn entry() -> MachineState {
    MachineState {
        open: Open::default(),
        elapsed: Elapsed(Some(0)),
    }
}

#[test]
fn capabilities_from_independent_crates_work_in_one_machine() {
    let program = module(vec![
        Inst::Seq(Seq::Open(1)),
        Inst::Clock(Clock::Wait(4)),
        Inst::Seq(Seq::Play(1)),
        Inst::Clock(Clock::Deadline(10)),
        Inst::Halt,
    ]);
    let options = VerifyOptions::default().with_type_maps(true);
    let (_, verification) = Verified::verify_with_state(program, entry(), options).unwrap();
    let maps = verification.type_maps.unwrap();
    let (_, state) = maps.get(0, 0).unwrap();
    assert_eq!(state.elapsed, Elapsed(Some(0)));

    // Each component's rule reports through the combined context.
    let errors = |code| {
        let failed = Verified::verify_with_state(module(code), entry(), VerifyOptions::default())
            .unwrap_err();
        failed
            .rejection
            .diagnostics
            .into_iter()
            .map(|d| (d.address, d.kind))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        errors(vec![Inst::Seq(Seq::Play(2)), Inst::Halt]),
        [(0, DiagnosticKind::Rule("channel 2 is not open".into()))]
    );
    assert_eq!(
        errors(vec![
            Inst::Clock(Clock::Wait(12)),
            Inst::Clock(Clock::Deadline(10)),
            Inst::Halt
        ]),
        [(
            1,
            DiagnosticKind::Rule("12 cycles have elapsed, over the budget of 10".into())
        )]
    );
}

#[test]
fn a_single_component_still_verifies_with_its_own_state() {
    // `Open` contains itself, so the sequencer works alone too.
    fn assert_channels<C: sequencer::Channels>() {}
    assert_channels::<WithState<'_, Open>>();
    assert_channels::<WithState<'_, MachineState>>();
    let program = module(vec![Inst::Seq(Seq::Open(1)), Inst::Halt]);
    // The machine's instruction set needs both capabilities.
    let flow = <Inst as Verify<WithState<'_, MachineState>>>::flow(&program.code[1]);
    assert_eq!(flow, Flow::Halt);
}
