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
    use vihaco_verifier::{
        ComponentState, ComponentStateOf, JoinPoint, Verify, VerifyError, WithState,
    };

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Open(pub BTreeSet<u32>);

    impl ComponentState for Open {
        /// Open after a join only if open on every path.
        fn join(&mut self, incoming: &Self, _: &JoinPoint) -> Result<bool, VerifyError> {
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

/// A timing component crate: an upper bound on elapsed cycles, widened at
/// loop headers and across calls.
mod timing {
    use vihaco_verifier::{
        ComponentState, ComponentStateOf, JoinPoint, Predecessor, Verify, VerifyError, WithState,
    };

    /// At most this many cycles have elapsed; `None` once unknown.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Elapsed(pub Option<u32>);

    impl ComponentState for Elapsed {
        /// Where paths merge the bound is the larger of the two. That chain
        /// could grow forever around a loop, so at a loop header (an edge
        /// jumping back to or before the join point) a disagreement widens to
        /// unknown instead: the join stays finite-height.
        fn join(&mut self, incoming: &Self, at: &JoinPoint) -> Result<bool, VerifyError> {
            let joined = match (self.0, incoming.0) {
                (Some(a), Some(b)) if a == b => Some(a),
                (Some(_), Some(_)) if is_back_edge(at) => None,
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            };
            let changed = joined != self.0;
            self.0 = joined;
            Ok(changed)
        }

        /// The callee is verified separately, so the caller cannot know how
        /// long it took: the bound becomes unknown.
        fn after_call(&mut self, _callee: Option<u32>, _entry: &Self) {
            self.0 = None;
        }
    }

    /// Whether either edge into `at` jumps back to or before it.
    fn is_back_edge(at: &JoinPoint) -> bool {
        [at.saved_from, at.incoming_from]
            .iter()
            .any(|edge| matches!(edge, Predecessor::Jump(from) if *from >= at.address))
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
                None => Err(VerifyError::rule(
                    "elapsed time is unknown here (after a loop or a call)",
                )),
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
    use vihaco_verifier::{Flow, Signatures, Type, TypedStack, Verify, component_state};

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
        /// Call the function starting at this address, with no arguments.
        Call(u32),
        /// Continue at either address (a loop's back edge and its exit).
        Loop(u32, u32),
        Ret,
        Halt,
    }

    impl<C: TypedStack + Signatures<Type> + ?Sized> Verify<C> for Inst
    where
        Seq: Verify<C>,
        Clock: Verify<C>,
    {
        fn flow(&self) -> Flow {
            match self {
                Inst::Halt => Flow::Halt,
                Inst::Ret => Flow::Return,
                Inst::Loop(back, exit) => Flow::jump_to([*back, *exit]),
                _ => Flow::Next,
            }
        }

        fn verify(&self, cx: &mut C) -> eyre::Result<()> {
            match self {
                Inst::Seq(seq) => seq.verify(cx),
                Inst::Clock(clock) => clock.verify(cx),
                Inst::Call(address) => vihaco_verifier::direct_call_effect(cx, 0, *address),
                Inst::Loop(..) | Inst::Ret | Inst::Halt => Ok(()),
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

/// `@main` is `code[..split]`, `@helper` the rest.
fn two_functions(code: Vec<Inst>, split: u32) -> LocalModule<Inst, Value, Type> {
    let mut module = module(code);
    let end = module.code.len() as u32;
    module.functions[0].end_address = split;
    let mut helper = module.functions[0].clone();
    helper.name = 1;
    helper.start_address = split;
    helper.end_address = end;
    module.functions.push(helper);
    module.strings.push("helper".into());
    module
}

fn rule_errors(module: LocalModule<Inst, Value, Type>) -> Vec<(u32, String)> {
    match Verified::verify_with_state(module, entry(), VerifyOptions::default()) {
        Ok(_) => vec![],
        Err(failed) => failed
            .rejection
            .diagnostics
            .into_iter()
            .map(|d| match d.kind {
                DiagnosticKind::Rule(message) => (d.address, message),
                kind => panic!("unexpected {kind:?}"),
            })
            .collect(),
    }
}

/// Calls are accounted for conservatively: the callee's time is unknown to
/// the caller, so a deadline after a call is rejected, and a channel opened
/// before a call is not assumed open after it (the callee might close it).
#[test]
fn component_state_does_not_survive_a_call() {
    let unknown = "elapsed time is unknown here (after a loop or a call)".to_owned();
    // main: wait 2; call helper; deadline 10.  helper: wait 50; ret.
    let timed = two_functions(
        vec![
            Inst::Clock(Clock::Wait(2)),
            Inst::Call(4),
            Inst::Clock(Clock::Deadline(10)),
            Inst::Halt,
            Inst::Clock(Clock::Wait(50)),
            Inst::Ret,
        ],
        4,
    );
    assert_eq!(rule_errors(timed), [(2, unknown)]);
    // A deadline checked before the call still verifies.
    let before = two_functions(
        vec![
            Inst::Clock(Clock::Wait(2)),
            Inst::Clock(Clock::Deadline(10)),
            Inst::Call(4),
            Inst::Halt,
            Inst::Clock(Clock::Wait(50)),
            Inst::Ret,
        ],
        4,
    );
    assert_eq!(rule_errors(before), []);
    // main: open 1; call helper; play 1.  helper: (nothing); ret.
    let channel = two_functions(
        vec![
            Inst::Seq(Seq::Open(1)),
            Inst::Call(4),
            Inst::Seq(Seq::Play(1)),
            Inst::Halt,
            Inst::Clock(Clock::Wait(0)),
            Inst::Ret,
        ],
        4,
    );
    assert_eq!(
        rule_errors(channel),
        [(2, "channel 1 is not open".to_owned())]
    );
}

/// The timing component uses the join point: paths that merge keep the
/// larger bound, while a loop header widens to unknown, so the fixpoint ends.
#[test]
fn joins_keep_the_larger_bound_and_widen_at_loop_headers() {
    // 0: wait 1; 1: loop -> 0 or 2; 2: deadline 100.
    let looping = module(vec![
        Inst::Clock(Clock::Wait(1)),
        Inst::Loop(0, 2),
        Inst::Clock(Clock::Deadline(100)),
        Inst::Halt,
    ]);
    assert_eq!(
        rule_errors(looping),
        [(
            2,
            "elapsed time is unknown here (after a loop or a call)".to_owned()
        )]
    );
    // 0: wait 3; 1: branch to 2 or 3 (a diamond, no back edge); 2: wait 4;
    // 3: deadline 6 sees max(3, 7) = 7.
    let diamond = module(vec![
        Inst::Clock(Clock::Wait(3)),
        Inst::Loop(2, 3),
        Inst::Clock(Clock::Wait(4)),
        Inst::Clock(Clock::Deadline(6)),
        Inst::Halt,
    ]);
    assert_eq!(
        rule_errors(diamond),
        [(3, "7 cycles have elapsed, over the budget of 6".to_owned())]
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
