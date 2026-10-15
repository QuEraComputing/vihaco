struct Local;
#[allow(non_upper_case_globals)]
impl Local {
    pub const __vihaco_field_a: usize = 0;
    pub const __vihaco_field_b: usize = 1;
}
impl vm::Field<0> for Local {
    type Child = consumer::Cpu<u8>;
    const SOURCE: &'static [&'static str] = &["a"];
}
impl vm::Field<1> for Local {
    type Child = consumer::Cpu<u8>;
    const SOURCE: &'static [&'static str] = &["b"];
}
impl vm::Capabilities for Local {
    const DESCRIPTION: &'static vm::Description = &vm::Description::Composite(&[
        vm::Device { name: "a", child: <consumer::Cpu<u8> as vm::Capabilities>::DESCRIPTION },
        vm::Device { name: "b", child: <consumer::Cpu<u8> as vm::Capabilities>::DESCRIPTION },
    ]);
}
#[declarations::instruction_set(composite = Local)]
enum Selected {
    #[target(a)] First(external::control::Halt),
    #[target(b)] Second(chain::again::control::Halt),
}
fn main() {}
