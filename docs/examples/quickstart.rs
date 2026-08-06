use eyre::Result;
use vihaco::{
    Effects, Execute, Execution, Instruction, Message, StepResult, expect_exactly_one_effect,
};

#[derive(Debug, Clone, Instruction)]
pub enum CounterInst {
    Add(i64),
    Print,
}

#[derive(Debug, Clone)]
pub struct Prefix(pub String);

impl Message for Prefix {}

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

fn main() -> Result<()> {
    let mut counter = Counter::default();

    // `Add` ignores its message and returns no effects.
    Execute::execute(&mut counter, &CounterInst::Add(2), Prefix(String::new()))?;
    Execute::execute(&mut counter, &CounterInst::Add(3), Prefix(String::new()))?;

    // `Print` returns exactly one `Line` effect.
    let effects = Execute::execute(
        &mut counter,
        &CounterInst::Print,
        Prefix("total = ".into()),
    )?
    .effects;
    let line = expect_exactly_one_effect(effects)?;
    assert_eq!(line, Line("total = 5".into()));
    Ok(())
}
