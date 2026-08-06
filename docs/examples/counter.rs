use eyre::Result;
use vihaco::{Effects, Execute, Execution, Instruction, Message, StepResult};

/// Bytecode-visible operations. Each variant becomes an opcode; tuple
/// fields become the payload bytes that follow it.
#[derive(Debug, Clone, Instruction)]
pub enum CounterInst {
    Add(i64),
    Print,
}

/// Resolved execution input — supplied by the runtime, not encoded in
/// the instruction stream.
#[derive(Debug, Clone)]
pub struct Prefix(pub String);

impl Message for Prefix {}

/// A value the component returns for the runtime or observers to consume.
#[derive(Debug, Clone, PartialEq)]
pub struct Line(pub String);

#[derive(Debug, Default)]
pub struct Counter {
    value: i64,
}

impl Counter {
    fn execute_instruction(&mut self, inst: &CounterInst, msg: Prefix) -> Result<Effects<Line>> {
        match inst {
            CounterInst::Add(v) => {
                self.value += v;
                Ok(Effects::none())
            }
            CounterInst::Print => Ok(Effects::one(Line(format!("{}{}", msg.0, self.value)))),
        }
    }
}

impl Execute<CounterInst> for Counter {
    type Message = Prefix;
    type Effect = Line;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        inst: &CounterInst,
        msg: Prefix,
    ) -> Result<StepResult<Line>> {
        Ok(StepResult {
            effects: self.execute_instruction(inst, msg)?,
            execution: Execution::Complete,
        })
    }
}
