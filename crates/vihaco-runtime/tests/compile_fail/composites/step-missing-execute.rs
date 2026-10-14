use vihaco_runtime::{Step, composite, field};

#[composite]
struct Child {}

#[composite]
struct Machine {
    #[device(100)]
    child: Child,
}

struct Unsupported;

impl Step<field!(Machine::child), Unsupported> for Machine {
    fn step(&mut self, _: Unsupported) -> vihaco_runtime::eyre::Result<()> { Ok(()) }
}

fn main() {}
