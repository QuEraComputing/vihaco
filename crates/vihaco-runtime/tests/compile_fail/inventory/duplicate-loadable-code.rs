use vihaco_runtime::composite;
#[composite] struct Machine {
    #[device(42)] ordinary_device: (),
    #[device(42)] #[loadable] image: (),
}
fn main() {}
