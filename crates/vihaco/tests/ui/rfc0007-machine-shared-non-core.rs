use eyre::Result;
use vihaco::{Effects, Instruction, Message, component};

#[derive(Instruction)]
enum DemoInst {
    Run,
}

#[derive(Message)]
struct DemoMsg;

struct DemoDevice;

#[component(instruction = DemoInst, message = DemoMsg)]
impl DemoDevice {
    fn execute(&mut self, _inst: DemoInst, _msg: DemoMsg) -> Result<Effects<()>> {
        Ok(Effects::none())
    }
}

#[vihaco::composite]
#[scheduler(device = 0x00, instruction = SchedulerInst)]
struct BadMachine {
    #[core]
    #[device(0x01)]
    core_a: DemoDevice,
    #[device(0x02)]
    observer_like: DemoDevice,
    #[shared(observer_like)]
    #[device(0x03)]
    device: DemoDevice,
}

#[derive(Instruction)]
enum SchedulerInst {
    Acquire,
    Release,
}

fn main() {}
