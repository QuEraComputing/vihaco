use eyre::Result;
use vihaco::{dispatch, Effects, Instruction, Message};

#[derive(Instruction)]
enum DemoInst {
    Run,
}

#[derive(Message)]
struct DemoMsg;

struct DemoDevice;

#[dispatch(instruction = DemoInst, message = DemoMsg)]
impl DemoDevice {
    fn execute(&mut self, _inst: &DemoInst, _msg: DemoMsg) -> Result<Effects<()>> {
        Ok(Effects::none())
    }
}

#[vihaco::composite]
struct BadMachine {
    #[device(0x01)]
    a: DemoDevice,
    #[device(0x01)]
    b: DemoDevice,
}

fn main() {}
