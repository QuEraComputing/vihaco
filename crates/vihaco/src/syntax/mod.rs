//! Parsed-module syntax — the source-text shape that `#[derive(Parse)]` and
//! the hand-written `Module`/`Function` parsers produce, **before** the
//! resolver lowers it into [`crate::module::Module`].
//!
//! Two-pass design: the parser yields [`ParsedModule`] holding [`BodyItem`]s
//! that are either fully-typed (`Direct`) or untyped source forms (`Raw`)
//! awaiting expansion / symbol resolution. Each consumer implements
//! [`Resolve`] for its instruction set to convert `Vec<BodyItem<I>>` into
//! `Vec<I>`.

mod types;

pub mod parse;
pub mod resolve;

pub use types::{BodyItem, Param, ParsedFunction, ParsedModule, RawForm, RawOperand, RawType};

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
    enum StubInst {
        Halt,
        Print,
    }

    #[derive(Debug, Clone, PartialEq, vihaco_parser::Parse)]
    #[head = "device "]
    enum StubHeader {
        #[token = "version"]
        #[delimiters(open = "", close = "", separator = "")]
        Version(u32),
    }

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
        // Each canonical instruction must own its full source line —
        // statement_end requires `\n` / `}` / EOF after a Direct match. That's
        // what lets the body parser distinguish `fpga::Play` (unit canonical)
        // from `fpga::Play 5` (sugar). Tests use real one-per-line layout.
        let src = "fn @main() {\n  halt\n  print\n  halt\n}";
        let f = ParsedFunction::<StubInst>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.body.len(), 3);
        assert_eq!(f.body[0], BodyItem::Direct(StubInst::Halt));
        assert_eq!(f.body[1], BodyItem::Direct(StubInst::Print));
        assert_eq!(f.body[2], BodyItem::Direct(StubInst::Halt));
    }

    #[test]
    fn body_item_falls_back_to_raw_for_unknown_mnemonic() {
        // `foo` isn't a StubInst variant; the body_item parser drops to raw_form.
        let src = "fn @main() { foo bar 1 2.0 }";
        let f = ParsedFunction::<StubInst>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.body.len(), 1);
        match &f.body[0] {
            BodyItem::Raw(raw) => {
                assert_eq!(raw.mnemonic, "foo");
                assert_eq!(raw.operands.len(), 3);
                assert_eq!(raw.operands[0], RawOperand::Ident("bar".into()));
                assert_eq!(raw.operands[1], RawOperand::UInt(1));
                assert_eq!(raw.operands[2], RawOperand::Float(2.0));
            }
            other => panic!("expected Raw, got {other:?}"),
        }
    }

    #[test]
    fn parses_return_type() {
        let src = "fn @main() -> i64 { halt }";
        let f = ParsedFunction::<StubInst>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.return_ty.as_ref().unwrap().0, "i64");
    }

    #[test]
    fn parses_module_with_headers_and_function() {
        let src = "\
device version 1;
fn @main() { halt }
";
        let m = ParsedModule::<StubInst, StubHeader>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(m.headers, vec![StubHeader::Version(1)]);
        assert_eq!(m.functions.len(), 1);
        assert_eq!(m.functions[0].body.len(), 1);
    }

    #[test]
    fn skips_line_comments() {
        let src = "\
fn @main() {
    // this is ignored
    halt
}
";
        let f = ParsedFunction::<StubInst>::parser()
            .parse(src)
            .into_result()
            .unwrap();
        assert_eq!(f.body.len(), 1);
    }

    #[test]
    fn body_item_handles_partial_match_then_fallback() {
        // `dump` is the only mnemonic Cnt accepts — `dump(42)` is canonical.
        // For input `dump foo` (no parens), Cnt's parser fails after consuming
        // `dump`, and choice has no other branch to try. `body_item` must
        // detect the failed direct branch and fall back to `raw_form`.
        //
        // No prefix-collision shortcut here: there's no shorter variant whose
        // token is a prefix of `dump` that could accidentally succeed.
        #[derive(Debug, Clone, PartialEq, vihaco_parser::Parse)]
        enum OnlyOne {
            #[delimiters(open = "(", close = ")", separator = "")]
            Dump(u32),
        }

        let src = "fn @main() { dump foo }";
        let f = ParsedFunction::<OnlyOne>::parser()
            .parse(src)
            .into_result()
            .unwrap_or_else(|e| panic!("parse failed: {e:?}"));
        assert_eq!(f.body.len(), 1, "body was: {:#?}", f.body);
        match &f.body[0] {
            BodyItem::Raw(raw) => {
                assert_eq!(raw.mnemonic, "dump");
                assert_eq!(raw.operands, vec![RawOperand::Ident("foo".into())]);
            }
            other => panic!("expected Raw fallback, got {other:?}"),
        }
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

    #[test]
    fn raw_operand_parses_symbol_string_and_negatives() {
        use crate::syntax::parse::raw_operand;

        let cases: Vec<(&str, RawOperand)> = vec![
            ("@body", RawOperand::Symbol("body".into())),
            ("\"hello\"", RawOperand::StringLit("hello".into())),
            ("-3", RawOperand::Int(-3)),
            ("-1.5", RawOperand::Float(-1.5)),
            ("AOD0:T1:A", RawOperand::Ident("AOD0:T1:A".into())),
        ];
        for (input, expected) in cases {
            let got = raw_operand().parse(input).into_result().unwrap();
            assert_eq!(got, expected, "input {input:?}");
        }
    }
}
