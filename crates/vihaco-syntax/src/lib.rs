// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Typed parsed-module syntax built from pattern-derived instruction and type
//! syntax.
//!
//! A [`ParsedModule`] contains [`ParsedFunction`] values whose bodies are
//! typed instruction vectors. Unknown or malformed instructions are parse
//! errors. Consumers implement [`Resolve`] to turn the typed parsed module
//! into the runtime module their machine loads.

mod types;

pub mod parse;
pub mod resolve;

pub use types::{ModuleSyntax, Param, ParsedFunction, ParsedModule};
pub use vihaco_parser::{InstructionSet, Parse, SurfaceInstruction};

pub use parse::{block_i64_flat, block_i64_pairs, skip};
pub use resolve::Resolve;

#[cfg(test)]
mod tests {
    use super::*;
    use chumsky::Parser as _;
    use vihaco_abi::traits::FromText;
    use vihaco_bytecode::SstHeader;
    use vihaco_parser::Parse;

    // Minimal stub: an enum that derives Parse and has just two unit variants.
    // Avoids pulling vihaco-cpu/-fpga into the test (cycle).
    #[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
    #[syntax_class(instruction)]
    enum StubInst {
        #[pattern = "'stub::halt"]
        Halt,
        #[pattern = "'stub::print"]
        Print,
    }

    #[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
    #[syntax_class(type)]
    enum StubType {
        #[pattern = "`unit`"]
        Unit,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct StubHeader;

    impl FromText for StubHeader {
        fn from_text(text: &str) -> eyre::Result<Self> {
            if text.trim().is_empty() {
                Ok(Self)
            } else {
                Err(eyre::eyre!("unexpected header text"))
            }
        }
    }

    impl SstHeader for StubHeader {}

    struct StubSyntax;

    impl ModuleSyntax for StubSyntax {
        type Instruction = StubInst;
        type Value = ();
        type Type = StubType;
        type Header = StubHeader;
    }

    #[test]
    fn parses_empty_function() {
        let src = "fn @main() {}";
        let f = ParsedFunction::<StubSyntax>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.name.as_str(), "main");
        assert!(f.body.is_empty());
    }

    #[test]
    fn parses_function_with_canonical_body() {
        let src = "fn @main() {\n  stub::halt\n  stub::print\n  stub::halt\n}";
        let f = ParsedFunction::<StubSyntax>::parser()
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
            ParsedFunction::<StubSyntax>::parser()
                .parse(src)
                .has_errors()
        );
    }

    #[test]
    fn parses_consumer_provided_return_type() {
        let src = "fn @main() -> unit { stub::halt }";
        let f = ParsedFunction::<StubSyntax>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.return_ty, Some(StubType::Unit));
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
    stub::halt
}
";
        let f = ParsedFunction::<StubSyntax>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.body.len(), 1);
    }

    #[test]
    fn rejects_malformed_known_instruction() {
        #[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
        #[syntax_class(instruction)]
        enum OnlyOne {
            #[pattern = "'stub::dump $0"]
            Dump(u32),
        }

        struct OnlyOneSyntax;

        impl ModuleSyntax for OnlyOneSyntax {
            type Instruction = OnlyOne;
            type Value = ();
            type Type = StubType;
            type Header = StubHeader;
        }

        let src = "fn @main() { stub::dump foo }";
        assert!(
            ParsedFunction::<OnlyOneSyntax>::parser()
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
