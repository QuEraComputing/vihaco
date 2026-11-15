use vihaco::module::LocalModule;
use vihaco::verifier::{VerifyOptions, verify_module};
use vihaco::{HasInstructionSet, Type, Value, component, composite, dialect};

#[dialect]
mod device {
    #[instruction]
    #[pattern = "'halt"]
    pub struct Halt;

    /// A leaf whose author forgot its verification rule.
    #[instruction]
    #[pattern = "'unverified"]
    pub struct Unverified;
}

impl<C: ?Sized> vihaco::Verify<C> for device::Halt {
    fn verify(&self, _: &mut C) -> vihaco::eyre::Result<()> {
        Ok(())
    }
}

#[component]
#[instructions { device }]
struct Device;

impl<I> vihaco::Execute<I> for Device {
    type Message = ();
    type Effect = ();
    type Error = std::convert::Infallible;
    fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[composite]
struct Machine {
    #[device(1)]
    device: Device,
}

impl<I> vihaco::Step<vihaco::field!(Machine::device), I> for Machine {
    fn step(&mut self, _: &I, _: &()) -> vihaco::eyre::Result<()> {
        Ok(())
    }
}

type Instruction = <Machine as HasInstructionSet>::Runtime;

fn main() {
    let module: LocalModule<Instruction, Value, Type> = LocalModule::default();
    let _ = verify_module(&module, VerifyOptions::default());
}
