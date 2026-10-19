// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::{component, composite, dialect, field, BuildMessage, Dispatch, Execute, NoMessage, Step};
#[dialect] mod ops { #[instruction] pub struct Run; }
#[component] #[instructions { ops }] struct Device;
impl Execute<ops::Run> for Device {
    type Message=NoMessage;type Effect=();type Error=eyre::Report;
    fn execute(&mut self,_:&ops::Run,_:NoMessage)->eyre::Result<()>{Ok(())}
}
#[composite] struct Machine { #[device(0)] device: Device }
impl Step<field!(Machine::device),ops::Run,u32> for Machine {
    fn step(&mut self,i:&ops::Run,context:&u32)->eyre::Result<()> {
        assert_eq!(*context,42);
        let message=<Self as BuildMessage<field!(Machine::device),ops::Run,u32>>::build_message(self,i,context)?; self.device.execute(i,message)
    }
}
fn main() {
    type Instruction=<Machine as vihaco_runtime::HasInstructionSet>::Runtime;
    let mut machine=Machine{device:Device};
    machine.dispatch(&Instruction::V_device__ops__Run(ops::Run),&"wrong context").unwrap();
}
