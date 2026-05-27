use eyre::Result;
use vihaco::{Instruction, Message, component, observe};

#[derive(Debug, Clone, Instruction)]
enum DemoInst {
    Ping,
}

#[derive(Message)]
struct DemoMsg;

#[derive(Debug, Clone)]
struct DemoEffect;

#[derive(Default)]
struct Sensor;

#[observe(DemoEffect)]
impl Sensor {
    fn observe_demo_effect(&mut self, _effect: &DemoEffect) -> eyre::Result<vihaco::Effects<()>> {
        Ok(vihaco::Effects::none())
    }
}

#[derive(Default)]
struct Device;

#[component(instruction = DemoInst, message = DemoMsg, effect = ())]
impl Device {
    fn execute(&mut self, _inst: DemoInst, _msg: DemoMsg) -> Result<vihaco::Effects<()>> {
        Ok(vihaco::Effects::none())
    }
}

fn build_snapshot(_sensor: &Sensor) {}

#[vihaco::composite]
struct BadMachine {
    #[observe(DemoEffect)]
    sensor: Sensor,

    #[device(0x01, observe(DemoEffect, ctx_with = build_snapshot(sensor)))]
    device: Device,
}

fn main() {}
