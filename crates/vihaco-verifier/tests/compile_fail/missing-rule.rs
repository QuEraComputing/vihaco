use vihaco_module::module::LocalModule;
use vihaco_verifier::{Type, Value, Verified};

/// An instruction with no verification rule at all.
pub struct Pulse(pub u32);

fn main() {
    let module: LocalModule<Pulse, Value, Type> = LocalModule::default();
    let _ = Verified::new(module);
}
