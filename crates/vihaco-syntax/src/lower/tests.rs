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

fn one_function_metadata() -> ProgramMetadata<Value, Type> {
    ProgramMetadata {
        structs: vec![],
        types: vec![],
        functions: vec![FunctionInfo {
            name: 0,
            signature: Signature {
                params: vec![],
                ret: vec![],
            },
            locals: vec![Type::U64, Type::F64],
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
    assert_eq!(module.functions[0].local_count(), 2);
    assert_eq!(module.extra, 7);
    Ok(())
}

/// Every header label must be declared in a body, with or without
/// `typed-declarations` (without it, a body declares none).
#[test]
fn a_header_label_without_a_declaration_is_rejected_in_every_build() {
    let mut metadata = one_function_metadata();
    metadata.labels = vec![vihaco_module::module::LabelInfo {
        address: 0,
        name: 0,
    }];
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
    assert_eq!(
        parsed.validate_metadata(&metadata).unwrap_err().to_string(),
        "metadata.labels[0]: label `@main` at address 0 is not declared in any function body"
    );
}

/// A `fn` line that disagrees with its metadata names the types by the
/// section's struct names, not by index.
#[test]
fn header_mismatches_show_types_by_name() {
    use vihaco_module::module::{FieldInfo, Parameter, StructInfo};
    let mut metadata = one_function_metadata();
    metadata
        .strings
        .extend(["p0", "Point", "Pair"].map(String::from));
    metadata.structs = vec![
        StructInfo {
            name: 3,
            fields: vec![],
        },
        StructInfo {
            name: 4,
            fields: vec![FieldInfo {
                name: 2,
                ty: Type::U64,
            }],
        },
    ];
    metadata.functions[0].signature.params = vec![Parameter {
        name: 2,
        ty: Type::Struct(1),
    }];
    metadata.functions[0].signature.ret = vec![Type::array(Type::Struct(1))];
    metadata.functions[0].locals = vec![Type::Struct(1)];
    let parsed = |param: Type, ret: Type| ParsedModule {
        header: (),
        functions: vec![ParsedFunction::<_, Type> {
            name: Ident("main".into()),
            params: vec![crate::Param {
                name: Ident("p0".into()),
                ty: param,
            }],
            return_types: vec![ret],
            body: vec![Literal(42)],
            #[cfg(feature = "typed-declarations")]
            labels: vec![],
        }],
    };
    let check = |module: ParsedModule<Literal, Type, ()>| {
        module.validate_metadata(&metadata).unwrap_err().to_string()
    };
    assert_eq!(
        check(parsed(Type::Struct(0), Type::array(Type::Struct(1)))),
        "function `@main`: parameter 0 is `p0: struct Point`, but its metadata declares \
         `p0: struct Pair`"
    );
    assert_eq!(
        check(parsed(Type::Struct(1), Type::Struct(1))),
        "function `@main` returns (struct Pair), but its metadata declares (array struct Pair)"
    );
}

#[cfg(feature = "program-image-execution")]
mod borrowed_metadata {
    use super::*;

    // Neither source instructions nor the prepared resource need to be Clone.
    struct PoolIndex(usize);
    struct Configuration(Box<[u32]>);
    struct Context<'a> {
        symbols: MetadataLoweringContext,
        configuration: &'a Configuration,
        entered: usize,
    }

    impl FunctionScope for Context<'_> {
        fn enter_function<'a>(
            &mut self,
            name: &str,
            labels: impl IntoIterator<Item = (&'a str, usize)>,
        ) -> eyre::Result<()> {
            self.entered += 1;
            self.symbols.enter_function(name, labels)
        }
    }

    impl Lower<Context<'_>> for PoolIndex {
        type Runtime = u32;

        fn lower(self, context: &mut Context<'_>) -> eyre::Result<u32> {
            context
                .configuration
                .0
                .get(self.0)
                .copied()
                .ok_or_else(|| eyre::eyre!("unknown pool index {}", self.0))
        }
    }

    fn functions(index: usize) -> Vec<ParsedFunction<PoolIndex, Type>> {
        vec![ParsedFunction {
            name: Ident("main".into()),
            params: vec![],
            return_types: vec![],
            body: vec![PoolIndex(index)],
            labels: vec![],
        }]
    }

    #[test]
    fn context_borrows_metadata_before_it_is_moved_into_the_module() -> eyre::Result<()> {
        let metadata = one_function_metadata().map_extra(|_| Configuration(Box::new([42])));
        let allocation = metadata.extra.0.as_ptr();
        let code = {
            let mut context = Context {
                symbols: MetadataLoweringContext::new(&metadata)?,
                configuration: &metadata.extra,
                entered: 0,
            };
            let code = lower_functions_with_metadata(functions(0), &metadata, &mut context)?;
            assert_eq!(context.entered, 1);
            code
        };
        let module = metadata.into_module(code)?;
        assert_eq!(module.code, [42]);
        assert_eq!(module.extra.0.as_ptr(), allocation);
        Ok(())
    }

    #[test]
    fn invalid_metadata_is_rejected_before_entering_a_function() -> eyre::Result<()> {
        let mut metadata = one_function_metadata().map_extra(|_| Configuration(Box::new([42])));
        metadata.functions[0].end_address = 2;
        let mut context = Context {
            symbols: MetadataLoweringContext::new(&metadata)?,
            configuration: &metadata.extra,
            entered: 0,
        };
        assert!(lower_functions_with_metadata(functions(0), &metadata, &mut context).is_err());
        assert_eq!(context.entered, 0);
        Ok(())
    }

    #[test]
    fn instruction_failures_retain_location_and_context_mutations() -> eyre::Result<()> {
        let metadata = one_function_metadata().map_extra(|_| Configuration(Box::new([42])));
        let mut context = Context {
            symbols: MetadataLoweringContext::new(&metadata)?,
            configuration: &metadata.extra,
            entered: 0,
        };
        let error =
            lower_functions_with_metadata(functions(1), &metadata, &mut context).unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains("lowering @main+0 (address 0): "),
            "{message}"
        );
        assert!(message.contains("unknown pool index 1"));
        assert_eq!(context.entered, 1);
        Ok(())
    }
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

    fn metadata() -> ProgramMetadata<Value, Type> {
        ProgramMetadata {
            structs: vec![],
            types: vec![],
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
                    locals: vec![Type::U64; 3],
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
                    locals: vec![Type::U64, Type::F64],
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

    fn context(metadata: &ProgramMetadata<Value, Type>) -> eyre::Result<CustomContext> {
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
    fn a_header_label_no_body_declares_is_rejected() -> eyre::Result<()> {
        // A label name instruction text could print, but source cannot write.
        let mut extra = metadata();
        extra.strings.push("p3".into());
        extra.labels.push(LabelInfo {
            address: 5,
            name: 5,
        });
        // A declared name at an address where no body declares it.
        let mut moved = metadata();
        moved.labels.insert(
            0,
            LabelInfo {
                address: 4,
                name: 2,
            },
        );
        for (metadata, expected) in [
            (
                extra,
                "metadata.labels[2]: label `@p3` at address 5 is not declared in any function body",
            ),
            (
                moved,
                "metadata.labels[0]: label `@done` at address 4 is not declared in any function \
                 body",
            ),
        ] {
            let mut context = context(&metadata)?;
            let error = parsed()
                .lower_with_metadata(metadata, &mut context)
                .unwrap_err();
            assert_eq!(error.to_string(), expected);
            assert!(context.entered.is_empty());
        }
        Ok(())
    }

    #[test]
    fn invalid_metadata_is_rejected_before_lowering_and_errors_retain_context() -> eyre::Result<()>
    {
        let mut metadata = metadata();
        let mut context = context(&metadata)?;
        metadata.functions[0].locals.clear();
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
            assert!(format!("{error:#}").contains("lowering @main+3 (address 6): "));
            assert!(format!("{error:#}").contains("missing"));
        }
        Ok(())
    }
}
