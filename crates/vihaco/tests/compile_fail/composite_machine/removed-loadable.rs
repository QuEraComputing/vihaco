struct Child;

#[vihaco::composite]
struct BadMachine {
    #[device(0x01)]
    #[loadable("child")]
    child: Child,
}

fn main() {}
