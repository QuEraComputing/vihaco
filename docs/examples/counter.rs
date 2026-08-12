use eyre::Result;
use vihaco::{component, Effects, Execute, Execution, StepResult};

component! {
    component Counter {
        value: i64,
    }

    runtime {
        instruction {
            Add(i64),
            Read,
        }
    }
}

// `component!` generates only the component and its instruction products.
// A containing `composite!` owns the instruction sum used for dispatch.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value(pub i64);

impl Execute<counter::runtime::instruction::Add> for counter::Counter {
    type Message = ();
    type Effect = ();
    type Fault = eyre::Report;

    fn execute(&mut self, instruction: &counter::runtime::instruction::Add, _: ()) -> Result<StepResult<()>> {
        self.value += instruction.0;
        Ok(StepResult { effects: Effects::none(), execution: Execution::Complete })
    }
}

impl Execute<counter::runtime::instruction::Read> for counter::Counter {
    type Message = ();
    type Effect = Value;
    type Fault = eyre::Report;

    fn execute(&mut self, _: &counter::runtime::instruction::Read, _: ()) -> Result<StepResult<Value>> {
        Ok(StepResult { effects: Effects::one(Value(self.value)), execution: Execution::Complete })
    }
}
