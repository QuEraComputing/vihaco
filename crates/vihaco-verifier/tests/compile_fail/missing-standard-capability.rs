use vihaco_verifier::{
    Frame, FrameState, FunctionSpan, Signatures, Type, TypeSlot, TypedStack, Verify,
    VerifyOptions, return_effect, verify_functions,
};

pub struct Ret;

impl<C: TypedStack + Signatures<Type> + ?Sized> Verify<C> for Ret {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        return_effect(cx, 0)
    }
}

fn main() {
    // A bare frame has no function table, so it cannot check returns.
    let entry = Frame::<TypeSlot>::new(vec![]);
    let span = FunctionSpan { function: 0, start: 0, end: 1, entry };
    let mut cx = FrameState::new(Frame::new(vec![]));
    let _ = verify_functions(&[Ret], &[span], &mut cx, VerifyOptions::default());
}
