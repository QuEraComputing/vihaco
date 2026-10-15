struct Local { r#type: consumer::Cpu<u8> }
#[allow(non_upper_case_globals)]
impl Local {
    pub const __vihaco_field_type: usize = 0;
}
impl vm::Field<0> for Local {
    type Child = consumer::Cpu<u8>;
    const SOURCE: &'static [&'static str] = &["type"];
}
impl vm::Capabilities for Local {
    const DESCRIPTION: &'static vm::Description = <consumer::Cpu<u8> as vm::Capabilities>::DESCRIPTION;
}
#[declarations::instruction_set(composite = Local)]
enum Selected { #[target(r#type)] Halt(external::control::Halt) }
fn main() {}
