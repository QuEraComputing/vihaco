use std::any::Any;

use eyre::Result;

#[doc(hidden)]
pub trait GeneratedMachine {
    type Instruction;

    fn metadata(&self) -> crate::runtime::CompositeMetadata;

    fn deliver_any(&mut self, effect: &dyn Any) -> Result<bool>;
}
