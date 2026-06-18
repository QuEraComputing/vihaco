use eyre::Result;
use vihaco::{Effects, Instruction, Message, component};

/// Bytecode-visible operations. Each variant becomes an opcode; tuple
/// fields become the payload bytes that follow it.
#[derive(Debug, Clone, Instruction)]
pub enum CounterInst {
    Add(i64),
    Print,
}

/// Resolved execution input — supplied by the runtime, not encoded in
/// the instruction stream.
#[derive(Debug, Clone, Message)]
pub struct Prefix(pub String);

/// A value the component returns for the runtime or observers to consume.
#[derive(Debug, Clone, PartialEq)]
pub struct Line(pub String);

#[derive(Debug, Default)]
pub struct Counter {
    value: i64,
}

// One `execute` per component: (instruction, message) in, effects out.
#[component(instruction = CounterInst, message = Prefix, effect = Line)]
impl Counter {
    fn execute(&mut self, inst: CounterInst, msg: Prefix) -> Result<Effects<Line>> {
        match inst {
            CounterInst::Add(v) => {
                self.value += v;
                Ok(Effects::none())
            }
            CounterInst::Print => Ok(Effects::one(Line(format!("{}{}", msg.0, self.value)))),
        }
    }
}
