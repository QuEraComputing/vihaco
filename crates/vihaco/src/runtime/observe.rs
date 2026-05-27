use crate::Effects;

pub trait Observe<E: 'static> {
    type Effect: 'static;
    type Error;

    fn observe(&mut self, effect: &E) -> Result<Effects<Self::Effect>, Self::Error>;
}
