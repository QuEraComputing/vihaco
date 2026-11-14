// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::*;
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_module::module::{FunctionDecl, NoInfo, Signature, SourceSymbolInfo};

#[derive(Debug)]
struct Literal(u32);

impl Lower<MetadataLoweringContext> for Literal {
    type Runtime = u32;

    fn lower(self, _: &mut MetadataLoweringContext) -> eyre::Result<u32> {
        Ok(self.0)
    }
}

fn decl(name: u32, local_count: u32, file: u32) -> FunctionDecl<vihaco_abi::Type> {
    FunctionDecl {
        name,
        signature: Signature {
            params: vec![],
            ret: vec![],
        },
        local_count,
        file,
    }
}

fn symbol(name: &str, address: u32) -> Symbol {
    Symbol {
        name: name.into(),
        address,
    }
}

fn one_function_metadata() -> ProgramMetadata {
    ProgramMetadata {
        functions: vec![decl(0, 2, 1)],
        constants: vec![],
        strings: vec!["main".into(), "test.sst".into()],
        main_function: Some(0),
        source_symbols: vec![],
        extra: NoInfo,
    }
}

#[test]
fn standard_loading_derives_function_ranges_from_symbols() -> eyre::Result<()> {
    let parsed = ParsedModule {
        header: (),
        code: vec![Literal(42)],
        symbols: vec![symbol("main", 0)],
    };
    let metadata = one_function_metadata();
    let mut context = MetadataLoweringContext::new(&metadata, &parsed.symbols)?;
    let module = parsed.lower_with_metadata(metadata.map_extra(|_| 7u8), &mut context)?;
    assert_eq!(module.code, [42]);
    assert_eq!(module.functions[0].local_count, 2);
    assert_eq!(
        (
            module.functions[0].start_address,
            module.functions[0].end_address
        ),
        (0, 1)
    );
    assert_eq!(module.symbols, [symbol("main", 0)]);
    assert_eq!(module.extra, 7);
    Ok(())
}

#[cfg(feature = "program-image-execution")]
mod borrowed_metadata {
    use super::*;

    // Neither source instructions nor the prepared resource need to be Clone.
    struct PoolIndex(usize);
    struct Configuration(Box<[u32]>);
    struct Context<'a> {
        configuration: &'a Configuration,
        lowered: usize,
    }

    impl Lower<Context<'_>> for PoolIndex {
        type Runtime = u32;

        fn lower(self, context: &mut Context<'_>) -> eyre::Result<u32> {
            context.lowered += 1;
            context
                .configuration
                .0
                .get(self.0)
                .copied()
                .ok_or_else(|| eyre::eyre!("unknown pool index {}", self.0))
        }
    }

    #[test]
    fn context_borrows_metadata_before_it_is_moved_into_the_module() -> eyre::Result<()> {
        let metadata = one_function_metadata().map_extra(|_| Configuration(Box::new([42])));
        let allocation = metadata.extra.0.as_ptr();
        let symbols = vec![symbol("main", 0)];
        let code = {
            let mut context = Context {
                configuration: &metadata.extra,
                lowered: 0,
            };
            let code =
                lower_code_with_metadata(vec![PoolIndex(0)], &symbols, &metadata, &mut context)?;
            assert_eq!(context.lowered, 1);
            code
        };
        let module = metadata.into_module(code, symbols)?;
        assert_eq!(module.code, [42]);
        assert_eq!(module.extra.0.as_ptr(), allocation);
        Ok(())
    }

    #[test]
    fn invalid_metadata_is_rejected_before_lowering() -> eyre::Result<()> {
        let metadata = one_function_metadata().map_extra(|_| Configuration(Box::new([42])));
        let mut context = Context {
            configuration: &metadata.extra,
            lowered: 0,
        };
        // `main` is declared but never defined as a symbol.
        let error =
            lower_code_with_metadata(vec![PoolIndex(0)], &[], &metadata, &mut context).unwrap_err();
        assert!(
            error.to_string().contains("has no `@main:` symbol"),
            "{error}"
        );
        assert_eq!(context.lowered, 0);
        Ok(())
    }

    #[test]
    fn instruction_failures_retain_location_and_context_mutations() -> eyre::Result<()> {
        let metadata = one_function_metadata().map_extra(|_| Configuration(Box::new([42])));
        let mut context = Context {
            configuration: &metadata.extra,
            lowered: 0,
        };
        let error = lower_code_with_metadata(
            vec![PoolIndex(0), PoolIndex(1)],
            &[symbol("main", 0)],
            &metadata,
            &mut context,
        )
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains("lowering instruction 1 (@main+1)"),
            "{message}"
        );
        assert!(message.contains("unknown pool index 1"));
        assert_eq!(context.lowered, 2);
        Ok(())
    }
}

mod symbols {
    use super::*;

    struct CustomContext {
        symbols: MetadataLoweringContext,
        multiplier: u32,
    }

    impl vihaco_module::loader::InternedStrings for CustomContext {
        fn take_strings(&mut self) -> Vec<String> {
            self.symbols.take_strings()
        }
    }

    #[derive(Clone)]
    enum Instruction {
        Address(&'static str),
        Function(&'static str),
        Label(&'static str),
        String(&'static str),
        Scale(u32),
    }

    impl Lower<CustomContext> for Instruction {
        type Runtime = u32;

        fn lower(self, context: &mut CustomContext) -> eyre::Result<u32> {
            match self {
                Self::Address(name) => context.symbols.function_address(name),
                Self::Function(name) => context.symbols.function_index(name),
                Self::Label(name) => context.symbols.label_address(name),
                Self::String(text) => context.symbols.string_index(text),
                Self::Scale(value) => Ok(value * context.multiplier),
            }
        }
    }

    /// ```text
    /// @helper:            // 0
    ///     label @helper.done
    ///     string "hello"
    /// @helper.done:       // 2
    ///     scale 7
    /// @main:              // 3
    ///     address @helper
    ///     function @helper
    ///     label @main.done
    /// @main.done:         // 6
    ///     scale 4
    /// ```
    fn parsed() -> ParsedModule<Instruction, ()> {
        ParsedModule {
            header: (),
            code: vec![
                Instruction::Label("helper.done"),
                Instruction::String("hello"),
                Instruction::Scale(7),
                Instruction::Address("helper"),
                Instruction::Function("helper"),
                Instruction::Label("main.done"),
                Instruction::Scale(4),
            ],
            symbols: vec![
                symbol("helper", 0),
                symbol("helper.done", 2),
                symbol("main", 3),
                symbol("main.done", 6),
            ],
        }
    }

    fn metadata() -> ProgramMetadata {
        ProgramMetadata {
            strings: vec![
                "main".into(),
                "helper".into(),
                "source.sst".into(),
                "hello".into(),
            ],
            // Table order differs from address order.
            functions: vec![decl(0, 3, 2), decl(1, 2, 2)],
            constants: vec![
                Value::U64(u64::MAX),
                Value::FunctionRef(1),
                Value::String(3),
            ],
            main_function: Some(0),
            source_symbols: vec![SourceSymbolInfo {
                index: 12,
                name: "user".into(),
            }],
            extra: NoInfo,
        }
    }

    fn context(metadata: &ProgramMetadata, symbols: &[Symbol]) -> eyre::Result<CustomContext> {
        Ok(CustomContext {
            symbols: MetadataLoweringContext::new(metadata, symbols)?,
            multiplier: 3,
        })
    }

    #[test]
    fn table_order_section_symbols_and_custom_capabilities_survive_loading() -> eyre::Result<()> {
        let metadata = metadata();
        let parsed = parsed();
        let mut context = context(&metadata, &parsed.symbols)?;
        let module = parsed.lower_with_metadata(metadata.clone(), &mut context)?;
        assert_eq!(module.code, [2, 3, 21, 0, 1, 6, 12]);
        assert_eq!(
            module
                .functions
                .iter()
                .map(|f| (f.name, f.local_count, f.start_address, f.end_address))
                .collect::<Vec<_>>(),
            // `@main` runs to the end; `@helper` stops at `@main`, not at its
            // own non-function `@helper.done` symbol.
            [(0, 3, 3, 7), (1, 2, 0, 3)]
        );
        assert_eq!(module.symbols, super::symbols::parsed().symbols);
        // `"hello"` was already in the table, so nothing was appended.
        assert_eq!(module.strings, metadata.strings);
        assert_eq!(module.constants, metadata.constants);
        assert_eq!(module.source_symbols, metadata.source_symbols);
        assert_eq!(module.main_function, metadata.main_function);
        Ok(())
    }

    #[test]
    fn invalid_metadata_is_rejected_before_lowering_and_errors_retain_location() -> eyre::Result<()>
    {
        let mut metadata = metadata();
        let parsed = parsed();
        let mut context = context(&metadata, &parsed.symbols)?;
        metadata.functions[0].local_count = 0;
        metadata.functions[0]
            .signature
            .params
            .push(vihaco_module::module::Parameter {
                name: 0,
                ty: vihaco_abi::Type::U64,
            });
        assert!(parsed.lower_with_metadata(metadata, &mut context).is_err());

        let metadata = super::symbols::metadata();
        let mut parsed = super::symbols::parsed();
        let mut context = super::symbols::context(&metadata, &parsed.symbols)?;
        parsed.code[4] = Instruction::Label("missing");
        let error = parsed
            .lower_with_metadata(metadata, &mut context)
            .unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains("lowering instruction 4 (@main+1)"),
            "{message}"
        );
        assert!(message.contains("missing"));
        Ok(())
    }

    #[test]
    fn lowering_interns_new_string_literals_after_the_header_strings() -> eyre::Result<()> {
        let metadata = metadata();
        let mut parsed = parsed();
        parsed.code[2] = Instruction::String("world");
        parsed.code[6] = Instruction::String("hello");
        let mut context = context(&metadata, &parsed.symbols)?;
        let module = parsed.lower_with_metadata(metadata.clone(), &mut context)?;
        assert_eq!(module.code[1..3], [3, 4]);
        assert_eq!(module.code[6], 3);
        assert_eq!(module.strings[..4], metadata.strings[..]);
        assert_eq!(module.strings[4..], ["world"]);
        Ok(())
    }

    #[test]
    fn duplicate_or_out_of_range_symbols_are_rejected() {
        let metadata = metadata();
        let mut parsed = parsed();
        parsed.symbols.push(symbol("main", 1));
        assert!(parsed.validate_metadata(&metadata).is_err());

        let mut parsed = super::symbols::parsed();
        parsed.symbols.push(symbol("late", 99));
        let error = parsed.validate_metadata(&metadata).unwrap_err();
        assert!(
            error.to_string().contains("outside the instruction stream"),
            "{error}"
        );
    }
}
