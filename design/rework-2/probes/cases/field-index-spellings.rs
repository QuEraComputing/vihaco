struct Local;
#[allow(non_upper_case_globals)]
impl Local {
    pub const __vihaco_field_cpu: usize = 1;
    pub const __vihaco_field_Cpu: usize = 0;
    pub const __vihaco_field_type: usize = 2;
    pub const __vihaco_field_λ: usize = 3;
}
macro_rules! field {
    ($index:literal, $name:literal) => {
        impl vm::Field<$index> for Local {
            type Child = consumer::Cpu<u8>;
            const SOURCE: &'static [&'static str] = &[$name];
        }
    };
}
field!(0, "Cpu");
field!(1, "cpu");
field!(2, "type");
field!(3, "λ");
impl vm::Capabilities for Local {
    const DESCRIPTION: &'static vm::Description =
        <consumer::Cpu<u8> as vm::Capabilities>::DESCRIPTION;
}
type Alias = Local;
#[declarations::instruction_set(composite = Alias)]
enum Selected {
    #[target(cpu)]
    Lower(external::control::Halt),
    #[target(Cpu)]
    Upper(chain::again::control::Halt),
    #[target(r#type)]
    Raw(external::control::Halt),
    #[target(λ)]
    Unicode(external::control::Halt),
}
fn main() {}
