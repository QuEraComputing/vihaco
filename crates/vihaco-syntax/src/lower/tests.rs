// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::*;
use crate::ParsedFunction;
use vihaco_abi::Type;
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_module::module::{FunctionInfo, NoInfo, Signature};
use vihaco_parser::Ident;

#[derive(Debug)]
struct Literal(u32);

impl Lower<MetadataLoweringContext> for Literal {
    type Runtime = u32;

    fn lower(self, _: &mut MetadataLoweringContext) -> eyre::Result<u32> {
        Ok(self.0)
    }
}

fn one_function_metadata() -> ProgramMetadata {
    ProgramMetadata {
        functions: vec![FunctionInfo {
            name: 0,
            signature: Signature {
                params: vec![],
                ret: vec![],
            },
            local_count: 2,
            start_address: 0,
            end_address: 1,
            file: 1,
        }],
        labels: vec![],
        constants: vec![],
        strings: vec!["main".into(), "test.sst".into()],
        main_function: Some(0),
        source_symbols: vec![],
        extra: NoInfo,
    }
}

#[test]
fn standard_loading_works_with_and_without_typed_declarations() -> eyre::Result<()> {
    let parsed = ParsedModule {
        header: (),
        functions: vec![ParsedFunction::<_, Type> {
            name: Ident("main".into()),
            params: vec![],
            return_types: vec![],
            body: vec![Literal(42)],
            #[cfg(feature = "typed-declarations")]
            labels: vec![],
        }],
    };
    let metadata = one_function_metadata();
    let mut context = MetadataLoweringContext::new(&metadata)?;
    let module = parsed.lower_with_metadata(metadata.map_extra(|_| 7u8), &mut context)?;
    assert_eq!(module.code, [42]);
    assert_eq!(module.functions[0].local_count, 2);
    assert_eq!(module.extra, 7);
    Ok(())
}

#[cfg(feature = "typed-declarations")]
mod typed {
    use super::*;
    use crate::ParsedLabel;
    use vihaco_module::module::{LabelInfo, SourceSymbolInfo};

    struct CustomContext {
        symbols: MetadataLoweringContext,
        multiplier: u32,
        entered: Vec<String>,
    }

    impl FunctionScope for CustomContext {
        fn enter_function<'a>(
            &mut self,
            name: &str,
            labels: impl IntoIterator<Item = (&'a str, usize)>,
        ) -> eyre::Result<()> {
            self.entered.push(name.to_owned());
            self.symbols.enter_function(name, labels)
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

    fn parsed() -> ParsedModule<Instruction, Type, ()> {
        ParsedModule {
            header: (),
            functions: vec![
                ParsedFunction {
                    name: Ident("helper".into()),
                    params: vec![],
                    return_types: vec![],
                    body: vec![
                        Instruction::Label("done"),
                        Instruction::String("hello"),
                        Instruction::Scale(7),
                    ],
                    labels: vec![ParsedLabel {
                        name: Ident("done".into()),
                        instruction_index: 2,
                    }],
                },
                ParsedFunction {
                    name: Ident("main".into()),
                    params: vec![],
                    return_types: vec![],
                    body: vec![
                        Instruction::Address("helper"),
                        Instruction::Function("helper"),
                        Instruction::Label("done"),
                        Instruction::Scale(4),
                    ],
                    labels: vec![ParsedLabel {
                        name: Ident("done".into()),
                        instruction_index: 3,
                    }],
                },
            ],
        }
    }

    fn metadata() -> ProgramMetadata {
        ProgramMetadata {
            strings: vec![
                "main".into(),
                "helper".into(),
                "done".into(),
                "source.sst".into(),
                "hello".into(),
            ],
            functions: vec![
                FunctionInfo {
                    name: 0,
                    signature: Signature {
                        params: vec![],
                        ret: vec![],
                    },
                    local_count: 3,
                    start_address: 3,
                    end_address: 7,
                    file: 3,
                },
                FunctionInfo {
                    name: 1,
                    signature: Signature {
                        params: vec![],
                        ret: vec![],
                    },
                    local_count: 2,
                    start_address: 0,
                    end_address: 3,
                    file: 3,
                },
            ],
            labels: vec![
                LabelInfo {
                    address: 2,
                    name: 2,
                },
                LabelInfo {
                    address: 6,
                    name: 2,
                },
            ],
            constants: vec![
                Value::U64(u64::MAX),
                Value::FunctionRef(1),
                Value::String(4),
            ],
            main_function: Some(0),
            source_symbols: vec![SourceSymbolInfo {
                index: 12,
                name: "user".into(),
            }],
            extra: NoInfo,
        }
    }

    fn context(metadata: &ProgramMetadata) -> eyre::Result<CustomContext> {
        Ok(CustomContext {
            symbols: MetadataLoweringContext::new(metadata)?,
            multiplier: 3,
            entered: vec![],
        })
    }

    #[test]
    fn table_order_scoped_labels_and_custom_capabilities_survive_loading() -> eyre::Result<()> {
        let metadata = metadata();
        let mut context = context(&metadata)?;
        let module = parsed().lower_with_metadata(metadata.clone(), &mut context)?;
        assert_eq!(module.code, [2, 4, 21, 0, 1, 6, 12]);
        assert_eq!(context.entered, ["helper", "main"]);
        assert_eq!(module.functions, metadata.functions);
        assert_eq!(module.labels, metadata.labels);
        assert_eq!(module.strings, metadata.strings);
        assert_eq!(module.constants, metadata.constants);
        assert_eq!(module.source_symbols, metadata.source_symbols);
        assert_eq!(module.main_function, metadata.main_function);
        Ok(())
    }

    #[test]
    fn invalid_metadata_is_rejected_before_lowering_and_errors_retain_context() -> eyre::Result<()>
    {
        let mut metadata = metadata();
        let mut context = context(&metadata)?;
        metadata.functions[0].local_count = 0;
        metadata.functions[0].end_address = 8;
        assert!(
            parsed()
                .lower_with_metadata(metadata, &mut context)
                .is_err()
        );
        assert!(context.entered.is_empty());
        for instruction in [
            Instruction::Label("missing"),
            Instruction::String("missing"),
        ] {
            let metadata = super::typed::metadata();
            let mut context = super::typed::context(&metadata)?;
            let mut parsed = parsed();
            parsed.functions[1].body[3] = instruction;
            let error = parsed
                .lower_with_metadata(metadata, &mut context)
                .unwrap_err();
            assert!(format!("{error:#}").contains("lowering function @main, instruction 3"));
            assert!(format!("{error:#}").contains("missing"));
        }
        Ok(())
    }
}
