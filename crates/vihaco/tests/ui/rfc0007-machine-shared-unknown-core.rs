use eyre::Result;
use vihaco::{Effects, Instruction, Message, component};

#[derive(Instruction)]
enum DemoInst {
    Run,
}

#[derive(Message)]
struct DemoMsg;

struct Core;

#[component(instruction = DemoInst, message = DemoMsg)]
impl Core {
    fn execute(&mut self, _inst: DemoInst, _msg: DemoMsg) -> Result<Effects<()>> {
        Ok(Effects::none())
    }
}

#[vihaco::composite]
#[scheduler(device = 0x00, instruction = SchedulerInst)]
struct BadMachine {
    #[core]
    #[device(0x01)]
    core_a: Core,
    #[shared(missing_core)]
    #[device(0x02)]
    device: Core,
}

#[derive(Instruction)]
enum SchedulerInst {
    Acquire,
    Release,
}

fn main() {}
