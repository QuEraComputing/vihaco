// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{BuildMessage, Execute, Step, component, composite, dialect, field};

#[dialect]
pub mod arithmetic {
    #[instruction]
    pub struct Add(pub usize, pub usize);

    #[instruction]
    pub struct Subtract(pub usize, pub usize);

    #[instruction]
    pub struct Read(pub usize);
}

pub mod message_kind {
    pub trait Binary {
        fn addresses(&self) -> (usize, usize);
    }

    pub trait Unary {
        fn address(&self) -> usize;
    }
}

impl message_kind::Binary for arithmetic::Add {
    fn addresses(&self) -> (usize, usize) {
        (self.0, self.1)
    }
}

impl message_kind::Binary for arithmetic::Subtract {
    fn addresses(&self) -> (usize, usize) {
        (self.0, self.1)
    }
}

impl message_kind::Unary for arithmetic::Read {
    fn address(&self) -> usize {
        self.0
    }
}

pub struct BinaryInput(pub u64, pub u64);

pub struct UnaryInput(pub u64);

#[component]
#[instructions { arithmetic }]
pub struct Calculator;

impl Execute<arithmetic::Add> for Calculator {
    type Message = BinaryInput;
    type Effect = u64;
    type Error = eyre::Report;

    fn execute(&mut self, _: &arithmetic::Add, input: BinaryInput) -> eyre::Result<u64> {
        input
            .0
            .checked_add(input.1)
            .ok_or_else(|| eyre::eyre!("addition overflow"))
    }
}

impl Execute<arithmetic::Subtract> for Calculator {
    type Message = BinaryInput;
    type Effect = u64;
    type Error = eyre::Report;

    fn execute(&mut self, _: &arithmetic::Subtract, input: BinaryInput) -> eyre::Result<u64> {
        input
            .0
            .checked_sub(input.1)
            .ok_or_else(|| eyre::eyre!("subtraction underflow"))
    }
}

impl Execute<arithmetic::Read> for Calculator {
    type Message = UnaryInput;
    type Effect = u64;
    type Error = eyre::Report;

    fn execute(&mut self, _: &arithmetic::Read, input: UnaryInput) -> eyre::Result<u64> {
        Ok(input.0)
    }
}

#[composite]
pub struct Machine {
    #[device(1)]
    calculator: Calculator,
    values: Vec<u64>,
    results: Vec<u64>,
}

impl Machine {
    fn value(&self, address: usize) -> eyre::Result<u64> {
        self.values
            .get(address)
            .copied()
            .ok_or_else(|| eyre::eyre!("unknown value address: {address}"))
    }
}

impl<I> BuildMessage<field!(Machine::calculator), I, (), BinaryInput> for Machine
where
    I: message_kind::Binary,
    Calculator: Execute<I, Message = BinaryInput>,
{
    fn build_message(&mut self, instruction: &I, _: &()) -> eyre::Result<BinaryInput> {
        let (lhs, rhs) = instruction.addresses();
        Ok(BinaryInput(self.value(lhs)?, self.value(rhs)?))
    }
}

impl<I> BuildMessage<field!(Machine::calculator), I, (), UnaryInput> for Machine
where
    I: message_kind::Unary,
    Calculator: Execute<I, Message = UnaryInput>,
{
    fn build_message(&mut self, instruction: &I, _: &()) -> eyre::Result<UnaryInput> {
        Ok(UnaryInput(self.value(instruction.address())?))
    }
}

impl<I> Step<field!(Machine::calculator), I> for Machine
where
    Calculator: Execute<I, Effect = u64, Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::calculator), I>,
{
    fn step(&mut self, instruction: &I, context: &()) -> eyre::Result<()> {
        let message = self.build_message(instruction, context)?;
        let result = self.calculator.execute(instruction, message)?;
        self.results.push(result);
        Ok(())
    }
}

pub fn main() -> eyre::Result<()> {
    let mut machine = Machine {
        calculator: Calculator,
        values: vec![20, 22],
        results: vec![],
    };
    type Destination = field!(Machine::calculator);

    <Machine as Step<Destination, arithmetic::Add>>::step(
        &mut machine,
        &arithmetic::Add(0, 1),
        &(),
    )?;
    <Machine as Step<Destination, arithmetic::Subtract>>::step(
        &mut machine,
        &arithmetic::Subtract(1, 0),
        &(),
    )?;
    <Machine as Step<Destination, arithmetic::Read>>::step(
        &mut machine,
        &arithmetic::Read(0),
        &(),
    )?;
    assert_eq!(machine.results, vec![42, 2, 20]);

    // An invalid address fails before execution adds another result.
    assert!(
        <Machine as Step<Destination, arithmetic::Read>>::step(
            &mut machine,
            &arithmetic::Read(99),
            &(),
        )
        .is_err()
    );
    assert_eq!(machine.results, vec![42, 2, 20]);
    Ok(())
}
