#[derive(framework::Message)]
pub struct DefaultRoot;
pub use framework as custom;
#[derive(framework::Message)]
#[vihaco(crate = self::custom)]
pub struct CustomRoot;
pub mod nested {
    #[derive(framework::Message)]
    #[vihaco(crate = super::custom)]
    pub struct ParentRoot;
    #[derive(framework::Message)]
    #[vihaco(crate = crate::custom)]
    pub struct CrateRoot;
}
fn require<T: framework::runtime::Message>() {}
pub fn check() {
    require::<DefaultRoot>();
    require::<CustomRoot>();
    require::<nested::ParentRoot>();
    require::<nested::CrateRoot>();
}

// Check the current default API directly; no compatibility alias is exported.
pub use framework::Parse;

pub struct Device;
#[derive(Clone, Debug, PartialEq)]
pub struct Surface;
impl<'src> framework::Parse<'src> for Surface {
    fn parser() -> impl framework::chumsky::Parser<
        'src,
        &'src str,
        Self,
        framework::chumsky::extra::Err<framework::chumsky::error::Simple<'src, char>>,
    > {
        use framework::chumsky::Parser as _;
        framework::chumsky::primitive::just("control.halt").to(Surface)
    }
}
impl framework::HasInstructionSet for Device {
    type Runtime = u8;
    type Syntax = Surface;
}
#[framework::composite]
pub struct DefaultMachine {
    #[device(0x01, alias = "alias")]
    pub device: Device,
}

#[test]
fn default_composite_preserves_parser_and_source_aliases() {
    use framework::chumsky::Parser as _;
    let parsed = default_machine::syntax::Instruction::parser()
        .parse("alias::control.halt")
        .into_result()
        .unwrap();
    assert_eq!(
        parsed,
        default_machine::syntax::Instruction::Device(Surface)
    );
}
