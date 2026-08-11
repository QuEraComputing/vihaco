// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

extern crate self as vihaco;

use vihaco_bytecode as binary;
// Extracted crates re-exported at their original module paths so the public
// API is unchanged (see design/crate-split.md §7).
pub use vihaco_abi::{effect, frame, instruction_syntax, metadata, program};
pub use vihaco_module::{color, loader, module, show, show_instruction};
pub use vihaco_syntax as syntax;
// The runtime layer and standard-library observers now live in their focused
// crates; re-export them at the original paths (see design/crate-split.md §7).
pub use vihaco_runtime as runtime;
#[doc(hidden)]
pub use vihaco_runtime::__private;
pub use vihaco_stdlib::observer;
pub mod instruction;
pub mod machine;
pub mod macros;
#[doc(hidden)]
pub mod traits;
pub mod value {
    pub use vihaco_abi::{Type, Value};
}

pub use binary::{
    BytecodeFile, BytecodeGlobalContext, BytecodeHeader, BytecodeSectionView, ConstantId,
    ContextHandle, FLAGS, GlobalContext, MAGIC, NoContext, SectionNameResolver, SectionPath,
    SstFile, SstGlobalContext, SstHeader, SstSectionView, VERSION, WriteBytecodeHeader,
    decode_instruction_stream,
};
pub use effect::Effects;
pub use instruction_syntax::{
    CanonicalInstructionSyntax, CanonicalInstructionVariantSyntax, InstructionSugarSyntax,
    InstructionSugarVariantSyntax, OperandKind, SugarOperandKind,
};
pub use loader::{
    BuildProgramModule, InstallProgramModule, LoadSstProgram, LoadSstSubtree, ProgramImage,
};
pub use macros::{Instruction, component, composite};
pub use program::{Type, Value};
pub use runtime::{
    Absorb, CompositeMetadata, EffectSink, Execute, Execution, Handle, Message,
    Message as MessageMarker, NoEffect, NoMessage, Observe, StepResult, Supply, complete,
    expect_exactly_one_effect,
};
pub use traits::{FromBytes, FromText, GetProgramInfo, Reset};
pub use vihaco_parser::{InstructionSet, Parse, SurfaceInstruction};
pub use vihaco_parser::{Parser, Simple, bare_token, extra, namespaced_parser};
pub use vihaco_parser_derive::Parse;
pub use vihaco_syntax::ModuleSyntax;

#[cfg(test)]
mod public_api_tests {
    use crate::{
        BytecodeGlobalContext, BytecodeHeader, ConstantId, EffectSink, Effects, Execute, Execution,
        GlobalContext, InstallProgramModule, ProgramImage, Reset, SectionNameResolver,
        SstGlobalContext, SstHeader, StepResult, WriteBytecodeHeader,
        instruction::{FromBytes, OpCode, WriteBytes},
        module::FunctionInfo,
        observer::stdio::StdoutEffect,
    };

    struct PublicReset;

    impl Reset for PublicReset {
        fn reset(&mut self) {}
    }

    struct PublicContext;

    impl SectionNameResolver for PublicContext {
        fn section_name(&self, _index: u32) -> Option<&str> {
            None
        }
    }

    impl BytecodeGlobalContext for PublicContext {
        fn from_bytes(_bytes: &[u8]) -> eyre::Result<Self> {
            Ok(Self)
        }
    }

    impl SstGlobalContext for PublicContext {
        fn from_text(_text: &str) -> eyre::Result<Self> {
            Ok(Self)
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
        fn require_install_program_module<T: InstallProgramModule<PublicContext>>() {}
        fn require_stdout_effect(_effect: StdoutEffect) {}
        fn require_metadata(_metadata: crate::CompositeMetadata) {}

        require_effect_sink::<Vec<()>>();
        require_reset::<PublicReset>();
        require_instruction::<u32>();
        require_bytecode_header::<u32>();
        require_sst_header::<PublicSstHeader>();
        require_write_bytecode_header::<u32>();
        require_section_name_resolver::<PublicContext>();
        require_bytecode_global_context::<PublicContext>();
        require_sst_global_context::<PublicContext>();
        require_sst_global_context::<crate::NoContext>();
        require_global_context::<PublicContext>();
        require_install_program_module::<ProgramImage<(), PublicContext>>();
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

    impl Execute<()> for DemoComponent {
        type Message = ();
        type Effect = u8;
        type Fault = eyre::Report;

        fn execute(
            &mut self,
            _inst: &(),
            _msg: Self::Message,
        ) -> eyre::Result<StepResult<Self::Effect>> {
            Ok(StepResult {
                effects: Effects::one(7),
                execution: Execution::Complete,
            })
        }
    }

    #[test]
    fn execute_component_without_exec_context() {
        let mut component = DemoComponent;
        let effects = Execute::execute(&mut component, &(), ()).unwrap().effects;

        assert_eq!(effects, Effects::one(7));
        assert_eq!(crate::expect_exactly_one_effect(effects).unwrap(), 7);
    }
}
