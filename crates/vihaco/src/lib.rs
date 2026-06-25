// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

extern crate self as vihaco;

#[doc(hidden)]
pub mod __private;
pub mod binary;
pub mod color;
pub mod effect;
pub mod frame;
pub mod instruction;
pub mod instruction_syntax;
pub mod loader;
pub mod machine;
pub mod macros;
pub mod metadata;
pub mod module;
pub mod observer;
pub mod runtime;
pub mod syntax;
#[doc(hidden)]
pub mod traits;
pub mod value;

pub use binary::{
    BytecodeContext, BytecodeFile, CompositeHeader, ConstantId, ContextHandle, ProgramContext,
    ProgramGlobals, SectionPath, SectionView,
};
pub use effect::Effects;
pub use instruction_syntax::{
    CanonicalInstructionSyntax, CanonicalInstructionVariantSyntax, InstructionSugarSyntax,
    InstructionSugarVariantSyntax, OperandKind, SugarOperandKind,
};
pub use loader::{LoadInput, LoadSection, ModuleProgramLoader, ProgramLoader};
pub use macros::{Instruction, Message, component, composite, observe};
pub use runtime::{
    CompositeMetadata, EffectSink, GeneratedComponent, Message as MessageMarker, Observe,
    expect_exactly_one_effect,
};
pub use traits::{GetProgramGlobal, Reset};
pub use value::{Type, Value};

#[cfg(test)]
mod public_api_tests {
    use crate::{
        BytecodeContext, EffectSink, Effects, GeneratedComponent, LoadSection, ProgramGlobals,
        Reset,
        binary::ConstantId,
        instruction::{FromBytes, OpCode, WriteBytes},
        module::FunctionInfo,
        observer::stdio::StdoutEffect,
    };

    struct PublicReset;

    impl Reset for PublicReset {
        fn reset(&mut self) {}
    }

    #[test]
    fn crate_root_exports_new_traits() {
        fn require_effect_sink<S: EffectSink<()>>() {}
        fn require_reset<T: Reset>() {}
        fn require_instruction<T: FromBytes + OpCode + WriteBytes>() {}
        fn require_bytecode_context<T: BytecodeContext>() {}
        fn require_program_globals<T: ProgramGlobals>() {}
        fn require_load_section<T: LoadSection>() {}
        fn require_stdout_effect(_effect: StdoutEffect) {}
        fn require_metadata(_metadata: crate::CompositeMetadata) {}

        require_effect_sink::<Vec<()>>();
        require_reset::<PublicReset>();
        require_instruction::<u32>();
        require_bytecode_context::<crate::ProgramContext>();
        require_program_globals::<crate::ProgramContext>();
        require_load_section::<crate::ProgramLoader<()>>();
        let _constant = ConstantId(0);
        let _function: Option<FunctionInfo<crate::Type>> = None;
        require_stdout_effect(StdoutEffect(String::new()));
        require_metadata(crate::CompositeMetadata {
            devices: &[],
            source_symbol_aliases: &[],
        });
    }

    #[derive(Clone, Copy)]
    struct DemoComponent;

    impl GeneratedComponent for DemoComponent {
        type Instruction = ();
        type Message = ();
        type Effect = u8;

        fn execute_generated(
            &mut self,
            _inst: Self::Instruction,
            _msg: Self::Message,
        ) -> eyre::Result<Effects<Self::Effect>> {
            Ok(Effects::one(7))
        }
    }

    #[test]
    fn generated_component_executes_without_exec_context() {
        let mut component = DemoComponent;
        let effects = GeneratedComponent::execute_generated(&mut component, (), ()).unwrap();

        assert_eq!(effects, Effects::one(7));
        assert_eq!(crate::expect_exactly_one_effect(effects).unwrap(), 7);
    }
}
