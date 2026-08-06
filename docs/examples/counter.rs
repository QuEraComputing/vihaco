use eyre::Result;
use vihaco::{component, Effects, Execute, Execution, StepResult};

component! {
    component Counter {
        value: i64,
    }

    instruction {
        Add(i64),
        Read,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value(pub i64);

impl Execute<counter::instruction::Add> for counter::Counter {
    type Message = ();
    type Effect = ();
    type Fault = eyre::Report;

    fn execute(&mut self, instruction: &counter::instruction::Add, _: ()) -> Result<StepResult<()>> {
        self.value += instruction.0;
        Ok(StepResult { effects: Effects::none(), execution: Execution::Complete })
    }
}

impl Execute<counter::instruction::Read> for counter::Counter {
    type Message = ();
    type Effect = Value;
    type Fault = eyre::Report;

    fn execute(&mut self, _: &counter::instruction::Read, _: ()) -> Result<StepResult<Value>> {
        Ok(StepResult { effects: Effects::one(Value(self.value)), execution: Execution::Complete })
    }
}
