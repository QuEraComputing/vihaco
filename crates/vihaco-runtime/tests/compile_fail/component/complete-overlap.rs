mod left {
    #[vihaco_runtime::dialect]
    pub mod shared { #[instruction] pub struct Halt; }
}
mod right {
    #[vihaco_runtime::dialect]
    pub mod shared { #[instruction] pub struct Halt; }
}
#[vihaco_runtime::component]
#[instructions { left::shared, right::shared }]
struct Cpu<T>(T);
impl<T, I> vihaco_runtime::Execute<I> for Cpu<T> {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;
    fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
fn main() {}
