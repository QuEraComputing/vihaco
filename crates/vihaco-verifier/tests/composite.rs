// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The engine verifies a real `#[composite]` machine through its generated
//! `Verify` aggregate. Run with `--features typed-declarations`.

#![cfg(feature = "typed-declarations")]

use std::collections::BTreeSet;

use vihaco_module::module::{FunctionInfo, LocalModule, Signature};
use vihaco_runtime::{HasInstructionSet, Lower as _, component, composite, dialect};
use vihaco_verifier::{
    AbstractState, ComponentState, DiagnosticKind, Flow, Frame, FrameState, FunctionSpan,
    ModuleVerifier, OperandStack, Type, TypeSlot, TypedSlot, Value, Verify, VerifyError,
    VerifyOptions, WithState, stack_effect, verify_functions,
};

/// A component-specific capability: which channels are open.
trait Channels {
    fn open(&mut self, channel: u32);
    fn require_open(&self, channel: u32) -> Result<(), VerifyError>;
}

#[dialect]
mod compute {
    #[instruction]
    #[pattern = "'const_i64"]
    pub struct ConstI64;

    #[instruction]
    #[pattern = "'const_f64"]
    pub struct ConstF64;

    #[instruction]
    #[pattern = "'const_bool"]
    pub struct ConstBool;

    #[instruction]
    #[pattern = "'add_i64"]
    pub struct AddI64;

    #[instruction]
    #[pattern = "'br $0"]
    pub struct Br(pub u32);

    #[instruction]
    #[pattern = "'br_if $0 `,` $1"]
    pub struct BranchIf(pub u32, pub u32);

    #[instruction]
    #[pattern = "'halt"]
    pub struct Halt;
}

#[dialect]
mod sequencer {
    #[instruction]
    #[pattern = "'open $0"]
    pub struct Open(pub u32);

    #[instruction]
    #[pattern = "'play $0"]
    pub struct Play(pub u32);
}

impl<C: OperandStack<Slot: TypedSlot> + ?Sized> Verify<C> for compute::ConstI64 {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        stack_effect(cx, [], [Type::I64])
    }
}

impl<C: OperandStack<Slot: TypedSlot> + ?Sized> Verify<C> for compute::ConstF64 {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        stack_effect(cx, [], [Type::F64])
    }
}

impl<C: OperandStack<Slot: TypedSlot> + ?Sized> Verify<C> for compute::ConstBool {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        stack_effect(cx, [], [Type::Bool])
    }
}

impl<C: OperandStack<Slot: TypedSlot> + ?Sized> Verify<C> for compute::AddI64 {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        stack_effect(cx, [Type::I64, Type::I64], [Type::I64])
    }
}

impl<C: ?Sized> Verify<C> for compute::Br {
    fn flow(&self) -> Flow {
        Flow::jump(self.0)
    }
    fn verify(&self, _: &mut C) -> eyre::Result<()> {
        Ok(())
    }
}

impl<C: OperandStack<Slot: TypedSlot> + ?Sized> Verify<C> for compute::BranchIf {
    fn flow(&self) -> Flow {
        Flow::jump_to([self.0, self.1])
    }
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        cx.pop_expect(&Type::Bool)?;
        Ok(())
    }
}

impl<C: ?Sized> Verify<C> for compute::Halt {
    fn flow(&self) -> Flow {
        Flow::Halt
    }
    fn verify(&self, _: &mut C) -> eyre::Result<()> {
        Ok(())
    }
}

impl<C: Channels + ?Sized> Verify<C> for sequencer::Open {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        cx.open(self.0);
        Ok(())
    }
}

impl<C: Channels + ?Sized> Verify<C> for sequencer::Play {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        Ok(cx.require_open(self.0)?)
    }
}

#[component]
#[instructions { compute }]
struct Cpu;

#[component]
#[instructions { sequencer }]
struct Sequencer;

impl<I> vihaco_runtime::Execute<I> for Cpu {
    type Message = ();
    type Effect = ();
    type Error = std::convert::Infallible;
    fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<I> vihaco_runtime::Execute<I> for Sequencer {
    type Message = ();
    type Effect = ();
    type Error = std::convert::Infallible;
    fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[composite]
struct Machine {
    #[device(1)]
    cpu: Cpu,
    #[device(2)]
    seq: Sequencer,
}

impl<I> vihaco_runtime::Step<vihaco_runtime::field!(Machine::cpu), I> for Machine {
    fn step(&mut self, _: &I, _: &()) -> eyre::Result<()> {
        Ok(())
    }
}

impl<I> vihaco_runtime::Step<vihaco_runtime::field!(Machine::seq), I> for Machine {
    fn step(&mut self, _: &I, _: &()) -> eyre::Result<()> {
        Ok(())
    }
}

/// The machine's verifier context: the standard frame plus open channels.
struct MachineVerifier {
    frame: FrameState<TypeSlot>,
    open: BTreeSet<u32>,
}

impl OperandStack for MachineVerifier {
    type Slot = TypeSlot;
    fn pop(&mut self) -> Result<TypeSlot, VerifyError> {
        self.frame.pop()
    }
    fn push(&mut self, slot: TypeSlot) {
        self.frame.push(slot);
    }
    fn peek(&self) -> Result<&TypeSlot, VerifyError> {
        self.frame.peek()
    }
    fn depth(&self) -> usize {
        self.frame.depth()
    }
}

impl Channels for MachineVerifier {
    fn open(&mut self, channel: u32) {
        self.open.insert(channel);
    }
    fn require_open(&self, channel: u32) -> Result<(), VerifyError> {
        if self.open.contains(&channel) {
            Ok(())
        } else {
            Err(VerifyError::rule(format!("channel {channel} is not open")))
        }
    }
}

impl AbstractState for MachineVerifier {
    type Snapshot = (Frame<TypeSlot>, BTreeSet<u32>);

    fn snapshot(&self) -> Self::Snapshot {
        (self.frame.snapshot(), self.open.clone())
    }

    fn restore(&mut self, (frame, open): &Self::Snapshot) {
        self.frame.restore(frame);
        self.open.clone_from(open);
    }

    fn join(
        (frame, open): &mut Self::Snapshot,
        (in_frame, in_open): &Self::Snapshot,
    ) -> Result<bool, VerifyError> {
        let frame_changed = frame.join(in_frame)?;
        let before = open.len();
        open.retain(|channel| in_open.contains(channel));
        Ok(frame_changed || open.len() != before)
    }

    fn set_address(&mut self, function: u32, address: u32) {
        self.frame.set_address(function, address);
    }
}

type Instruction = <Machine as HasInstructionSet>::Runtime;

fn parse(source: &str) -> Instruction {
    use vihaco_runtime::{Parse, chumsky::Parser as _};
    <Machine as HasInstructionSet>::Syntax::parser()
        .parse(source)
        .into_result()
        .unwrap_or_else(|errors| panic!("{source}: {errors:?}"))
        .lower(&mut ())
        .unwrap()
}

fn verify(program: &[&str]) -> Result<(), Vec<(u32, DiagnosticKind)>> {
    let code: Vec<Instruction> = program.iter().map(|source| parse(source)).collect();
    let entry = (Frame::new(vec![]), BTreeSet::new());
    let mut cx = MachineVerifier {
        frame: FrameState::new(entry.0.clone()),
        open: BTreeSet::new(),
    };
    let function = FunctionSpan {
        function: 0,
        start: 0,
        end: code.len() as u32,
        entry,
    };
    verify_functions(&code, &[function], &mut cx, VerifyOptions::default())
        .map(|_| ())
        .map_err(|rejection| {
            rejection
                .diagnostics
                .into_iter()
                .map(|d| (d.address, d.kind))
                .collect()
        })
}

#[test]
fn the_generated_aggregate_verifies_a_well_typed_program() {
    verify(&[
        "cpu::compute::const_bool", // 0
        "cpu::compute::br_if 2, 5", // 1
        "seq::sequencer::open 1",   // 2
        "cpu::compute::const_i64",  // 3
        "cpu::compute::br 7",       // 4
        "seq::sequencer::open 1",   // 5
        "cpu::compute::const_i64",  // 6
        "seq::sequencer::play 1",   // 7: join, channel 1 open on both arms
        "cpu::compute::const_i64",  // 8
        "cpu::compute::add_i64",    // 9
        "cpu::compute::halt",       // 10
    ])
    .unwrap();
}

#[test]
fn the_generated_aggregate_reports_each_components_errors() {
    let errors = verify(&[
        "cpu::compute::const_bool", // 0
        "cpu::compute::br_if 2, 5", // 1
        "seq::sequencer::open 1",   // 2
        "cpu::compute::const_i64",  // 3
        "cpu::compute::br 7",       // 4
        "cpu::compute::const_f64",  // 5: channel 1 not opened on this arm
        "cpu::compute::br 7",       // 6
        "seq::sequencer::play 1",   // 7: join
        "cpu::compute::halt",       // 8
    ])
    .unwrap_err();
    assert_eq!(
        errors,
        [(7, DiagnosticKind::Rule("channel 1 is not open".into()))]
    );

    let errors = verify(&[
        "cpu::compute::const_bool", // 0
        "cpu::compute::br_if 2, 4", // 1
        "cpu::compute::const_i64",  // 2
        "cpu::compute::br 5",       // 3
        "cpu::compute::const_f64",  // 4
        "cpu::compute::const_i64",  // 5: join
        "cpu::compute::add_i64",    // 6: i64 | f64 + i64
        "cpu::compute::halt",       // 7
    ])
    .unwrap_err();
    assert_eq!(
        errors,
        [(
            6,
            DiagnosticKind::TypeMismatch {
                expected: "i64".into(),
                found: "i64 | f64".into()
            }
        )]
    );
}

/// The same component state for the standard module context: the sequencer's
/// open channels, joined by intersection.
#[derive(Clone, Debug, Default)]
struct OpenChannels(BTreeSet<u32>);

impl ComponentState for OpenChannels {
    fn join(&mut self, incoming: &Self) -> Result<bool, VerifyError> {
        let before = self.0.len();
        self.0.retain(|channel| incoming.0.contains(channel));
        Ok(self.0.len() != before)
    }
}

impl Channels for WithState<'_, OpenChannels> {
    fn open(&mut self, channel: u32) {
        self.state_mut().0.insert(channel);
    }
    fn require_open(&self, channel: u32) -> Result<(), VerifyError> {
        if self.state().0.contains(&channel) {
            Ok(())
        } else {
            Err(VerifyError::rule(format!("channel {channel} is not open")))
        }
    }
}

#[test]
fn the_generated_aggregate_verifies_through_a_wrapped_module_verifier() {
    let verify = |program: &[&str]| {
        let code: Vec<Instruction> = program.iter().map(|source| parse(source)).collect();
        let module: LocalModule<Instruction, Value, Type> = LocalModule {
            functions: vec![FunctionInfo {
                name: 0,
                signature: Signature {
                    params: vec![],
                    ret: vec![],
                },
                locals: vec![],
                start_address: 0,
                end_address: code.len() as u32,
                file: 0,
            }],
            code,
            strings: vec!["main".into()],
            main_function: Some(0),
            ..Default::default()
        };
        let mut cx = ModuleVerifier::new(&module).with_state(OpenChannels::default());
        cx.verify(&module.code, VerifyOptions::default())
            .map(|_| ())
            .map_err(|rejection| {
                rejection
                    .diagnostics
                    .into_iter()
                    .map(|d| (d.address, d.kind))
                    .collect::<Vec<_>>()
            })
    };
    verify(&[
        "seq::sequencer::open 1", // 0
        "seq::sequencer::play 1", // 1
        "cpu::compute::halt",     // 2
    ])
    .unwrap();
    assert_eq!(
        verify(&[
            "cpu::compute::const_bool", // 0
            "cpu::compute::br_if 2, 4", // 1
            "seq::sequencer::open 1",   // 2
            "cpu::compute::br 4",       // 3
            "seq::sequencer::play 1",   // 4: join, open on one path only
            "cpu::compute::halt",       // 5
        ])
        .unwrap_err(),
        [(4, DiagnosticKind::Rule("channel 1 is not open".into()))]
    );
}
