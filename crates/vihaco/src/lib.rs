// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

extern crate self as vihaco;

#[doc(hidden)]
pub mod __private;
mod binary;
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
pub mod program;
pub mod runtime;
pub mod syntax;
#[doc(hidden)]
pub mod traits;
pub mod value {
    pub use crate::program::{Type, Value};
}

pub use binary::{
    BytecodeFile, BytecodeGlobalContext, BytecodeHeader, BytecodeSectionView, ConstantId,
    ContextHandle, FLAGS, GlobalContext, MAGIC, NoHeader, SectionNameResolver, SectionPath,
    SstFile, SstGlobalContext, SstHeader, SstSectionView, VERSION, WriteBytecodeHeader,
    decode_instruction_stream, parse_instruction_stream,
};
pub use effect::Effects;
pub use instruction_syntax::{
    CanonicalInstructionSyntax, CanonicalInstructionVariantSyntax, InstructionSugarSyntax,
    InstructionSugarVariantSyntax, OperandKind, SugarOperandKind,
};
pub use loader::{
    BytecodeLoadInput, LoadBytecodeSection, LoadOwnBytecodeSection, LoadOwnSstSection,
    LoadSstSection, ModuleProgramLoader, ProgramLoader, SstLoadInput,
};
pub use macros::{Instruction, Message, component, composite, observe};
pub use program::{ProgramContext, ProgramGlobals, Type, Value};
pub use runtime::{
    CompositeMetadata, EffectSink, GeneratedComponent, Message as MessageMarker, Observe,
    expect_exactly_one_effect,
};
pub use traits::{FromBytes, FromText, GetProgramGlobal, Reset};

#[cfg(test)]
mod public_api_tests {
    use crate::{
        BytecodeGlobalContext, BytecodeHeader, ConstantId, EffectSink, Effects, GeneratedComponent,
        GlobalContext, LoadBytecodeSection, LoadOwnBytecodeSection, ProgramGlobals, Reset,
        SectionNameResolver, SstGlobalContext, SstHeader, WriteBytecodeHeader,
        instruction::{FromBytes, OpCode, WriteBytes},
        module::FunctionInfo,
        observer::stdio::StdoutEffect,
    };

    struct PublicReset;

    impl Reset for PublicReset {
        fn reset(&mut self) {}
    }

    impl LoadOwnBytecodeSection for PublicReset {
        fn load_own_bytecode_section<'bc>(
            &mut self,
            _input: crate::BytecodeLoadInput<'bc>,
        ) -> eyre::Result<()> {
            Ok(())
        }
    }

    struct PublicSstHeader;

    impl crate::traits::FromText for PublicSstHeader {
        fn from_text(_text: &str) -> eyre::Result<Self> {
            Ok(Self)
        }
    }

    impl SstHeader for PublicSstHeader {}

    #[test]
    fn crate_root_exports_new_traits() {
        fn require_effect_sink<S: EffectSink<()>>() {}
        fn require_reset<T: Reset>() {}
        fn require_instruction<T: FromBytes + OpCode + WriteBytes>() {}
        fn require_bytecode_header<T: BytecodeHeader>() {}
        fn require_sst_header<T: SstHeader>() {}
        fn require_write_bytecode_header<T: WriteBytecodeHeader>() {}
        fn require_section_name_resolver<T: SectionNameResolver>() {}
        fn require_bytecode_global_context<T: BytecodeGlobalContext>() {}
        fn require_sst_global_context<T: SstGlobalContext>() {}
        fn require_global_context<T: GlobalContext>() {}
        fn require_program_globals<T: ProgramGlobals>() {}
        fn require_load_own_bytecode_section<T: LoadOwnBytecodeSection>() {}
        fn require_load_bytecode_section<T: LoadBytecodeSection>() {}
        fn require_stdout_effect(_effect: StdoutEffect) {}
        fn require_metadata(_metadata: crate::CompositeMetadata) {}

        require_effect_sink::<Vec<()>>();
        require_reset::<PublicReset>();
        require_instruction::<u32>();
        require_bytecode_header::<u32>();
        require_sst_header::<PublicSstHeader>();
        require_write_bytecode_header::<u32>();
        require_section_name_resolver::<crate::ProgramContext>();
        require_bytecode_global_context::<crate::ProgramContext>();
        require_sst_global_context::<crate::ProgramContext>();
        require_sst_global_context::<crate::NoHeader>();
        require_global_context::<crate::ProgramContext>();
        require_program_globals::<crate::ProgramContext>();
        require_load_own_bytecode_section::<PublicReset>();
        require_load_bytecode_section::<crate::ProgramLoader<()>>();
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
