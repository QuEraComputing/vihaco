use eyre::Result;
use vihaco::{component, dispatch, Effects, Message};

// Bytecode-visible operations. Each variant becomes an opcode; tuple fields
// become the payload bytes that follow it.
component! {
    #[derive(Debug, Default)]
    pub component Counter {
        value: i64,
    }

    instruction {
        Add(i64),
        Print,
    }
}

use counter::runtime::Instruction as CounterInst;

/// Resolved execution input — supplied by the runtime, not encoded in
/// the instruction stream.
#[derive(Debug, Clone, Message)]
pub struct Prefix(pub String);

/// A value the component returns for the runtime or observers to consume.
#[derive(Debug, Clone, PartialEq)]
pub struct Line(pub String);

// One `execute` per component: (instruction, message) in, effects out.
#[dispatch(instruction = counter::runtime::Instruction, message = Prefix, effect = Line)]
impl counter::Counter {
    fn execute(&mut self, inst: &CounterInst, msg: Prefix) -> Result<Effects<Line>> {
        match inst {
            CounterInst::Add(v) => {
                self.value += v;
                Ok(Effects::none())
            }
            CounterInst::Print => Ok(Effects::one(Line(format!("{}{}", msg.0, self.value)))),
        }
    }
}
