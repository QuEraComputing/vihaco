use vihaco_runtime::composite;
#[composite] struct Machine<T: vihaco_runtime::__private::Capabilities> { #[device(1)] cpu: T }
fn main() {}
