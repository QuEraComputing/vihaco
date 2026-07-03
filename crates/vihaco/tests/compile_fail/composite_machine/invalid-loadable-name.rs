struct Child;

#[vihaco::composite]
struct BadMachine {
    #[device(0x01)]
    #[loadable("child/nested")]
    child: Child,
}

fn main() {}
