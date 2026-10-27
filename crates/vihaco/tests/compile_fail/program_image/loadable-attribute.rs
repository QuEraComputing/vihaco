use vihaco::composite;

#[composite]
struct Machine {
    #[loadable]
    image: (),
}

fn main() {}
