mod nested {
    pub mod plain {}
}
#[vihaco_runtime::component]
#[instructions { nested::plain }]
struct Cpu;
fn main() {}
