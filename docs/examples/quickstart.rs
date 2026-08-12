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

// The component owns these product structs; a composite assembles them into
// its machine-local instruction enum when it is used in a machine.

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

fn main() -> Result<()> {
    let mut counter = counter::Counter { value: 0 };
    Execute::execute(&mut counter, &counter::runtime::instruction::Add(5), ())?;
    let value = Execute::execute(&mut counter, &counter::runtime::instruction::Read, ())?
        .effects
        .into_iter()
        .next()
        .expect("Read emits one value");
    assert_eq!(value, Value(5));
    Ok(())
}
