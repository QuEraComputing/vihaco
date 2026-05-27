pub trait EffectSink<E> {
    fn emit(&mut self, effect: E);
}

impl<E> EffectSink<E> for () {
    #[inline(always)]
    fn emit(&mut self, _effect: E) {}
}

impl<E> EffectSink<E> for Vec<E> {
    #[inline(always)]
    fn emit(&mut self, effect: E) {
        self.push(effect);
    }
}
