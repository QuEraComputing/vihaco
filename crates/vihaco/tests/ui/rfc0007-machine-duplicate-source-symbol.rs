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
    #[device(0x01, alias = "scheduler")]
    device: DemoDevice,
}

#[derive(Instruction)]
enum SchedulerInst {
    Acquire,
    Release,
}

fn main() {}
