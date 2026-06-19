use eyre::Result;
use vihaco::{
    Effects, GeneratedComponent, Instruction, Message, component, expect_exactly_one_effect,
};

#[derive(Debug, Clone, Instruction)]
pub enum CounterInst {
    Add(i64),
    Print,
}

#[derive(Debug, Clone, Message)]
pub struct Prefix(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct Line(pub String);

#[derive(Debug, Default)]
pub struct Counter {
    value: i64,
}

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

fn main() -> Result<()> {
    let mut counter = Counter::default();

    // `Add` ignores its message and returns no effects.
    counter.execute_generated(CounterInst::Add(2), Prefix(String::new()))?;
    counter.execute_generated(CounterInst::Add(3), Prefix(String::new()))?;

    // `Print` returns exactly one `Line` effect.
    let effects = counter.execute_generated(CounterInst::Print, Prefix("total = ".into()))?;
    let line = expect_exactly_one_effect(effects)?;
    assert_eq!(line, Line("total = 5".into()));
    Ok(())
}
