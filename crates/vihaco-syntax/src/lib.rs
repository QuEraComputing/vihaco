// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Typed SST parsing built from pattern-derived instruction and type syntax.
//!
//! Section owners select their header type and combine device declarations with
//! the derived `vihaco_module::module::ProgramMetadata` record. The existing
//! [`ParsedModule`] flow retains `fn` bodies and can validate them against supplied
//! metadata. Instructions use their existing `Lower<C>` implementations.
//! [`ParsedModule::lower_with_metadata`] combines validation, function-scope
//! preparation, instruction lowering, and module assembly. Its context can use
//! `vihaco_module::loader::MetadataLoweringContext` directly or add consumer
//! capabilities by implementing `vihaco_module::loader::FunctionScope`.
//!
//! With `program-image-execution`, `HeaderDeclarations<D>` parses a collection
//! of semicolon-terminated declarations. Consumers apply declarations separately
//! to an application-owned preparation context, then call
//! `lower_functions_with_metadata` with the completed metadata and instruction
//! context. This supports contexts borrowing prepared configuration without
//! reconstructing a parsed module or moving its metadata during body lowering.
//!
//! Metadata records use `#[derive(Parse)]` and
//! `#[syntax_class(metadata, open = "{", close = "}")]` with an optional `head`.
//! See `vihaco-stdlib/examples/loadable.rs` for header declarations, metadata
//! validation, and execution using dialect instructions consumed by components.
//!
//! A [`ParsedModule`] contains [`ParsedFunction`] values whose bodies are
//! typed instruction vectors. Unknown or malformed instructions are parse
//! errors. With `typed-declarations`, bodies can contain the generated syntax
//! aggregate, lowered with a program loader's capabilities before execution.
//! Labels retain their instruction positions without emitting executable slots.
//! Feature-off consumers retain
//! the separate `Resolve` bridge.

mod lower;
mod types;

pub use lower::lower_functions;

#[cfg(feature = "program-image-execution")]
mod header;

#[cfg(feature = "program-image-execution")]
pub use header::HeaderDeclarations;

#[cfg(feature = "program-image-execution")]
pub use lower::lower_functions_with_metadata;

pub mod parse;
#[cfg(not(feature = "typed-declarations"))]
pub mod resolve;

#[cfg(feature = "typed-declarations")]
pub use types::ParsedLabel;
pub use types::{Param, ParsedFunction, ParsedModule};
pub use vihaco_parser::SurfaceInstruction;

pub use parse::{block_i64_flat, block_i64_pairs, skip};
#[cfg(not(feature = "typed-declarations"))]
pub use resolve::Resolve;

#[cfg(test)]
mod tests {
    use super::*;
    use chumsky::Parser as _;
    use vihaco_parser::Parse;

    // Minimal stub: an enum that derives Parse and has just two unit variants.
    // Avoids pulling vihaco-cpu/-fpga into the test (cycle).
    #[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
    #[syntax_class(instruction, head = "stub")]
    enum StubInst {
        Halt,
        Print,
    }

    #[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
    #[syntax_class(type)]
    enum StubType {
        #[pattern = "`unit`"]
        Unit,
    }

    #[test]
    fn parses_empty_function() {
        let src = "fn @main() {}";
        let f = ParsedFunction::<StubInst, StubType>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.name.as_str(), "main");
        assert!(f.params.is_empty());
        assert!(f.body.is_empty());
    }

    #[cfg(feature = "typed-declarations")]
    #[test]
    fn retains_label_positions_without_executable_slots() {
        let f = ParsedFunction::<StubInst, StubType>::parser()
            .parse("fn @main() { label @entry label @alias stub.halt label @exit }")
            .into_result()
            .unwrap();
        assert_eq!(f.body, [StubInst::Halt]);
        assert_eq!(
            f.labels
                .iter()
                .map(|label| (label.name.as_str(), label.instruction_index))
                .collect::<Vec<_>>(),
            [("entry", 0), ("alias", 0), ("exit", 1)],
        );
    }

    #[test]
    fn parses_function_parameters() {
        let src = "fn @main(input: unit, output: unit) {}";
        let f = ParsedFunction::<StubInst, StubType>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(
            f.params,
            vec![
                Param {
                    name: vihaco_parser::Ident("input".to_owned()),
                    ty: StubType::Unit,
                },
                Param {
                    name: vihaco_parser::Ident("output".to_owned()),
                    ty: StubType::Unit,
                },
            ]
        );
    }

    #[test]
    fn parses_function_parameters_with_whitespace() {
        let src = "fn @main( input : unit , output:unit ) {}";
        let f = ParsedFunction::<StubInst, StubType>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[0].name.as_str(), "input");
        assert_eq!(f.params[1].name.as_str(), "output");
    }

    #[test]
    fn rejects_parameter_without_type() {
        let src = "fn @main(input) {}";
        assert!(
            ParsedFunction::<StubInst, StubType>::parser()
                .parse(src)
                .has_errors()
        );
    }

    #[test]
    fn parses_function_with_canonical_body() {
        let src = "fn @main() {\n  stub.halt\n  stub.print\n  stub.halt\n}";
        let f = ParsedFunction::<StubInst, StubType>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(
            f.body,
            vec![StubInst::Halt, StubInst::Print, StubInst::Halt]
        );
    }

    #[test]
    fn rejects_unknown_instruction() {
        let src = "fn @main() { foo bar 1 2.0 }";
        assert!(
            ParsedFunction::<StubInst, StubType>::parser()
                .parse(src)
                .has_errors()
        );
    }

    #[test]
    fn parses_consumer_provided_return_type() {
        let src = "fn @main() -> unit { stub.halt }";
        let f = ParsedFunction::<StubInst, StubType>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.return_types, vec![StubType::Unit]);
    }

    #[test]
    fn lexical_helpers_return_explicit_newtypes() {
        assert_eq!(
            parse::symbol_ref().parse("@main").into_result(),
            Ok(vihaco_parser::Ident("main".to_owned()))
        );
        assert_eq!(
            parse::string_literal()
                .parse("\"hello\\nworld\"")
                .into_result(),
            Ok(vihaco_parser::QuotedString("hello\nworld".to_owned()))
        );
    }

    #[test]
    fn skips_line_comments() {
        let src = "\
fn @main() {
    // this is ignored
    stub.halt
}
";
        let f = ParsedFunction::<StubInst, StubType>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.body.len(), 1);
    }

    #[test]
    fn rejects_malformed_known_instruction() {
        #[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
        #[syntax_class(instruction, head = "stub")]
        enum OnlyOne {
            Dump(u32),
        }

        let src = "fn @main() { stub.dump foo }";
        assert!(
            ParsedFunction::<OnlyOne, StubType>::parser()
                .parse(src)
                .has_errors()
        );
    }

    #[test]
    fn block_i64_flat_parses_whitespace_separated_ints() {
        let got = block_i64_flat().parse(" 0 1 2 3 ").into_result().unwrap();
        assert_eq!(got, vec![0, 1, 2, 3]);
    }

    #[test]
    fn block_i64_pairs_parses_rows() {
        let got = block_i64_pairs()
            .parse("\n  1 1\n  5 1\n  1 5\n")
            .into_result()
            .unwrap();
        assert_eq!(got, vec![(1, 1), (5, 1), (1, 5)]);
    }

    #[test]
    fn block_i64_flat_accepts_empty() {
        let got = block_i64_flat().parse("").into_result().unwrap();
        assert!(got.is_empty());
    }
}
