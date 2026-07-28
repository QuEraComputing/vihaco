// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Typed parsed-module syntax produced by `#[derive(Parse)]` and the
//! hand-written module/function parsers.
//!
//! A [`ParsedModule`] contains [`ParsedFunction`] values whose bodies are
//! typed instruction vectors. Unknown or malformed instructions are parse
//! errors. Consumers implement [`Resolve`] to turn the typed parsed module
//! into the runtime module their machine loads.

mod types;

pub mod parse;
pub mod resolve;

pub use types::{
    Param, ParsedFunction, ParsedModule, SurfaceInstruction, SurfaceType, SurfaceValue,
};

pub use parse::{block_i64_flat, block_i64_pairs, skip};
pub use resolve::Resolve;

#[cfg(test)]
mod tests {
    use super::*;
    use chumsky::Parser as _;
    use vihaco_parser_core::Parse;

    // Minimal stub: an enum that derives Parse and has just two unit variants.
    // Avoids pulling vihaco-cpu/-fpga into the test (cycle).
    #[derive(Debug, Clone, PartialEq, vihaco_parser::Parse)]
    #[syntax_class(instruction, head = "stub")]
    enum StubInst {
        Halt,
        Print,
    }

    impl SurfaceInstruction for StubInst {}

    #[test]
    fn parses_empty_function() {
        let src = "fn @main() {}";
        let f = ParsedFunction::<StubInst>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.name, "main");
        assert!(f.body.is_empty());
    }

    #[test]
    fn parses_function_with_canonical_body() {
        let src = "fn @main() {\n  stub::halt\n  stub::print\n  stub::halt\n}";
        let f = ParsedFunction::<StubInst>::parser()
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
        assert!(ParsedFunction::<StubInst>::parser().parse(src).has_errors());
    }

    #[test]
    fn parses_return_type() {
        let src = "fn @main() -> i64 { stub::halt }";
        let f = ParsedFunction::<StubInst>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        let expected = SurfaceType::parser().parse("i64").into_result().unwrap();
        assert_eq!(f.return_ty, Some(expected));
    }

    #[test]
    fn skips_line_comments() {
        let src = "\
fn @main() {
    // this is ignored
    stub::halt
}
";
        let f = ParsedFunction::<StubInst>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.body.len(), 1);
    }

    #[test]
    fn rejects_malformed_known_instruction() {
        #[derive(Debug, Clone, PartialEq, vihaco_parser::Parse)]
        #[syntax_class(instruction, head = "stub")]
        enum OnlyOne {
            Dump(u32),
        }

        impl SurfaceInstruction for OnlyOne {}

        let src = "fn @main() { stub::dump foo }";
        assert!(ParsedFunction::<OnlyOne>::parser().parse(src).has_errors());
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
