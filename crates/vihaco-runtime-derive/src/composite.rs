// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod codegen;
mod loadable;
mod metadata;
mod syntax;
mod validate;

use proc_macro::TokenStream;
use syntax::CompositeDeclaration;

pub fn expand(input: TokenStream) -> TokenStream {
    let declaration = syn::parse_macro_input!(input as CompositeDeclaration);
    match codegen::try_expand(declaration) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::CompositeDeclaration;
    use syn::parse_str;

    #[test]
    fn parses_runtime_routes_and_handlers() {
        let declaration: CompositeDeclaration = parse_str(
            r#"
                composite Cpu {
                    error = CpuFault;
                    stack: Stack,
                    alu: Alu,
                    debug: Debug,
                }
                runtime {
                    Add(AddInstruction) => alu {
                        message from stack;
                        effects {
                            observe debug;
                            absorb with stack;
                        }
                    }
                    Recv(RecvInstruction) => alu {
                        message with resolve_message;
                        effects {
                            handle with handle_receive;
                        }
                    }
                }
            "#,
        )
        .unwrap();

        assert_eq!(declaration.routes.len(), 2);
        assert!(matches!(
            declaration.routes[0].message,
            super::syntax::MessageSource::From(_)
        ));
        assert!(matches!(
            declaration.routes[0].handler,
            Some(super::syntax::Handler::Absorb(_))
        ));
        assert!(matches!(
            declaration.routes[1].handler,
            Some(super::syntax::Handler::With(_))
        ));
    }

    #[test]
    fn parses_structural_composites_without_an_error_or_routes() {
        let declaration: CompositeDeclaration =
            parse_str(r#"composite Machine { clock: Clock, }"#).unwrap();
        assert!(declaration.error.is_none());
        assert!(declaration.routes.is_empty());
    }

    #[test]
    fn parses_routes_without_effects_blocks() {
        let declaration: CompositeDeclaration = parse_str(
            r#"
                composite CounterMachine {
                    error = CounterMachineFault;
                    counter_group: CounterGroup,
                }
                runtime {
                    Queue(QueueInstruction) => counter_group {
                        message none;
                    }
                }
            "#,
        )
        .unwrap();

        assert_eq!(declaration.routes.len(), 1);
        assert!(declaration.routes[0].observers.is_empty());
        assert!(declaration.routes[0].handler.is_none());
    }

    #[test]
    fn parses_syntax_entries_and_direct_runtime_mappings() {
        let declaration: CompositeDeclaration = parse_str(
            r#"
                composite Machine {
                    error = MachineFault;
                    device: Device,
                }
                syntax {
                    #[pattern = "'device::clear"]
                    Clear => runtime Clear;
                    #[pattern = "'device::set $0"]
                    Set(u32) => lower_set;
                }
                runtime {
                    Clear(DeviceInstruction) => device {
                        message none;
                    }
                }
            "#,
        )
        .unwrap();

        assert_eq!(declaration.syntax.len(), 2);
        assert!(matches!(
            declaration.syntax[0].mapping,
            super::syntax::SyntaxMapping::Runtime(_)
        ));
        assert!(matches!(
            declaration.syntax[1].mapping,
            super::syntax::SyntaxMapping::Lower(_)
        ));
    }
}
