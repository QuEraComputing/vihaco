use vihaco_runtime::composite;
trait Choice { type Child: vihaco_runtime::__private::Capabilities; }
#[composite] struct Machine<T: Choice> { #[device(1)] cpu: T::Child }
fn main() {}
