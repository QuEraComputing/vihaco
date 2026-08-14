use eyre::Result;
use vihaco::{
    component, dispatch, Effects, GeneratedComponent, Message, expect_exactly_one_effect,
};

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

#[derive(Debug, Clone, Message)]
pub struct Prefix(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct Line(pub String);

#[dispatch(instruction = counter::runtime::Instruction, message = Prefix, effect = Line)]
impl counter::Counter {
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

fn main() -> Result<()> {
    let mut counter = counter::Counter::default();

    // `Add` ignores its message and returns no effects.
    counter.execute_generated(CounterInst::Add(2), Prefix(String::new()))?;
    counter.execute_generated(CounterInst::Add(3), Prefix(String::new()))?;

    // `Print` returns exactly one `Line` effect.
    let effects = counter.execute_generated(CounterInst::Print, Prefix("total = ".into()))?;
    let line = expect_exactly_one_effect(effects)?;
    assert_eq!(line, Line("total = 5".into()));
    Ok(())
}
