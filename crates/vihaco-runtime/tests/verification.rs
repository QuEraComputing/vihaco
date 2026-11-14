// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Instruction-owned `Verify` rules and the composite aggregate that routes to them.

#![cfg(feature = "typed-declarations")]

use vihaco_runtime::{Flow, HasInstructionSet, Verify, component, composite, dialect};

/// A verifier capability: the operand stack's depth.
trait OperandDepth {
    fn pop(&mut self, n: usize) -> eyre::Result<()>;
    fn push(&mut self, n: usize);
}

/// A component-specific capability: which channels are open.
trait Channels {
    fn open(&mut self, channel: u32);
    fn require_open(&self, channel: u32) -> eyre::Result<()>;
}

#[dialect]
mod compute {
    #[instruction]
    #[pattern = "'push"]
    pub struct Push;

    #[instruction]
    #[pattern = "'add"]
    pub struct Add;

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

impl<C: OperandDepth + ?Sized> Verify<C> for compute::Push {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        cx.push(1);
        Ok(())
    }
}

impl<C: OperandDepth + ?Sized> Verify<C> for compute::Add {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        cx.pop(2)?;
        cx.push(1);
        Ok(())
    }
}

impl<C: OperandDepth + ?Sized> Verify<C> for compute::BranchIf {
    fn flow(&self) -> Flow {
        Flow::jump_to([self.0, self.1])
    }
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        cx.pop(1)
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
        cx.require_open(self.0)
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

/// A machine-specific verifier context providing both capabilities.
#[derive(Default)]
struct Verifier {
    depth: usize,
    open: Vec<u32>,
}

impl OperandDepth for Verifier {
    fn pop(&mut self, n: usize) -> eyre::Result<()> {
        self.depth = self
            .depth
            .checked_sub(n)
            .ok_or_else(|| eyre::eyre!("stack underflow"))?;
        Ok(())
    }
    fn push(&mut self, n: usize) {
        self.depth += n;
    }
}

impl Channels for Verifier {
    fn open(&mut self, channel: u32) {
        self.open.push(channel);
    }
    fn require_open(&self, channel: u32) -> eyre::Result<()> {
        eyre::ensure!(
            self.open.contains(&channel),
            "channel {channel} is not open"
        );
        Ok(())
    }
}

type Instruction = <Machine as HasInstructionSet>::Runtime;

fn parse(source: &str) -> Instruction {
    use vihaco_runtime::{Parse, chumsky::Parser as _};
    <Machine as HasInstructionSet>::Syntax::parser()
        .parse(source)
        .into_result()
        .unwrap()
        .lower(&mut ())
        .unwrap()
}

use vihaco_runtime::Lower as _;

#[test]
fn the_aggregate_routes_each_instruction_to_its_component_rule() {
    let mut verifier = Verifier::default();
    let program = [
        "cpu::compute::push",
        "cpu::compute::push",
        "cpu::compute::add",
        "seq::sequencer::open 3",
        "seq::sequencer::play 3",
    ];
    for source in program {
        let instruction = parse(source);
        assert_eq!(flow(&instruction), Flow::Next);
        instruction.verify(&mut verifier).unwrap();
    }
    assert_eq!(verifier.depth, 1);
    let branch = parse("cpu::compute::br_if 4, 9");
    assert_eq!(flow(&branch), Flow::jump_to([4, 9]));
    branch.verify(&mut verifier).unwrap();
    assert_eq!(verifier.depth, 0);
    assert_eq!(flow(&parse("cpu::compute::halt")), Flow::Halt);
}

/// The aggregate's control flow, which needs no verifier state.
fn flow(instruction: &Instruction) -> Flow {
    <Instruction as Verify<Verifier>>::flow(instruction)
}

#[test]
fn control_flow_is_known_even_where_the_rule_would_fail() {
    // An empty stack makes `br_if`'s rule fail, but its targets are fixed.
    let branch = parse("cpu::compute::br_if 2, 5");
    assert_eq!(flow(&branch), Flow::jump_to([2, 5]));
    assert!(branch.verify(&mut Verifier::default()).is_err());
}

#[test]
fn rule_errors_propagate_from_every_component() {
    let mut verifier = Verifier::default();
    let err = parse("cpu::compute::add")
        .verify(&mut verifier)
        .unwrap_err();
    assert!(err.to_string().contains("stack underflow"), "{err}");
    let err = parse("seq::sequencer::play 1")
        .verify(&mut verifier)
        .unwrap_err();
    assert!(err.to_string().contains("channel 1 is not open"), "{err}");
}

#[cfg(not(feature = "program-image-execution"))]
#[test]
fn verification_capability_diagnostics() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/verification/*.rs");
}
