#[vihaco::composite]
struct Machine<T> {
    #[device(1)]
    child: Outer::<T>::Device,
}
fn main() {}
