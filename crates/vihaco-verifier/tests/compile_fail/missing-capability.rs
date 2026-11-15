use vihaco_module::module::LocalModule;
use vihaco_verifier::{Type, Value, Verify, VerifyError, VerifyOptions, verify_module};

/// A component capability the standard context does not provide.
pub trait Channels {
    fn require_open(&self, channel: u32) -> Result<(), VerifyError>;
}

pub struct Play(pub u32);

impl<C: Channels + ?Sized> Verify<C> for Play {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        Ok(cx.require_open(self.0)?)
    }
}

fn main() {
    let module: LocalModule<Play, Value, Type> = LocalModule::default();
    let _ = verify_module(&module, VerifyOptions::default());
}
