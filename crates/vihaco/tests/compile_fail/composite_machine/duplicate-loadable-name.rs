struct Child;

vihaco::composite! {
composite BadMachine {
    #[device(0x01)]
    #[loadable("child")]
    a: Child,
    #[device(0x02)]
    #[loadable("child")]
    b: Child,
}
}

fn main() {}
