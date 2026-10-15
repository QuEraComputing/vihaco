#[vihaco::composite]
struct Machine {
    #[cfg(any())]
    #[device(1)]
    child: Missing,
}
fn main() {}
