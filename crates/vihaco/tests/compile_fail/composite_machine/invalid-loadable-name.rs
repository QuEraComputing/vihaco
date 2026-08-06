struct Child;

vihaco::composite! {
composite BadMachine {
    #[device(0x01)]
    #[loadable("child/nested")]
    child: Child,
}
}

fn main() {}
