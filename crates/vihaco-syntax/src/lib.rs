// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Typed SST parsing built from pattern-derived instruction and type syntax.
//!
//! A section's `.text` is a flat instruction stream, not structured code:
//!
//! ```text
//! @main:
//! execution::program::call 0, @helper
//! execution::program::ret 1
//! @helper:
//! execution::program::const_u64 21
//! execution::program::ret 1
//! ```
//!
//! `@name:` binds a symbol to the address of the next instruction and occupies
//! no slot; `@name` in an operand refers to a symbol and is resolved through the
//! section's symbol table during lowering. Layout and indentation carry no
//! meaning. What a symbol *is* lives in the header: a function is a symbol with
//! a matching declaration in `vihaco_module::module::ProgramMetadata`, which
//! supplies its signature and locals. Its executable range runs from its symbol
//! to the next function symbol.
//!
//! [`ParsedModule::parse_section`] yields the header, the code stream, and the
//! symbol table. [`ParsedModule::lower_with_metadata`] validates them against
//! the metadata, lowers instructions with their `Lower<C>` implementations,
//! and assembles a module. Its context can use
//! `vihaco_module::loader::MetadataLoweringContext` directly or wrap it to add
//! consumer capabilities.
//!
//! With `program-image-execution`, `HeaderDeclarations<D>` parses a collection
//! of semicolon-terminated declarations. Consumers apply declarations separately
//! to an application-owned preparation context, then call
//! [`lower_code_with_metadata`] with the completed metadata and instruction
//! context. This supports contexts borrowing prepared configuration without
//! moving the metadata during lowering.
//!
//! Metadata records use `#[derive(Parse)]` and
//! `#[syntax_class(metadata, open = "{", close = "}")]` with an optional `head`.
//! See `vihaco-stdlib/examples/loadable.rs` for header declarations, metadata
//! validation, and execution using dialect instructions consumed by components.
//! Feature-off consumers retain the separate `Resolve` bridge.

mod lower;
mod types;

pub use lower::lower_code;

#[cfg(feature = "program-image-execution")]
mod header;

#[cfg(feature = "program-image-execution")]
pub use header::HeaderDeclarations;

#[cfg(feature = "program-image-execution")]
pub use lower::lower_code_with_metadata;

pub mod parse;
#[cfg(not(feature = "typed-declarations"))]
pub mod resolve;

pub use types::{ParsedModule, Symbol};
pub use vihaco_parser::SurfaceInstruction;

pub use parse::{block_i64_flat, block_i64_pairs, skip, symbol_definition};
#[cfg(not(feature = "typed-declarations"))]
pub use resolve::Resolve;

#[cfg(test)]
mod tests {
    use super::*;
    use chumsky::Parser as _;

    // Minimal stub: an enum that derives Parse and has just two unit variants.
    // Avoids pulling vihaco-cpu/-fpga into the test (cycle).
    #[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
    #[syntax_class(instruction, head = "stub")]
    enum StubInst {
        Halt,
        Print,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct NoHeader;

    impl vihaco_abi::traits::FromText for NoHeader {
        fn from_text(_: &str) -> eyre::Result<Self> {
            Ok(Self)
        }
    }

    impl vihaco_bytecode::SSTHeader for NoHeader {}

    fn parse(text: &str) -> eyre::Result<ParsedModule<StubInst, NoHeader>> {
        let source = format!("sst v2\n.section main\n.text\n{text}");
        let file = vihaco_bytecode::SSTFile::<vihaco_bytecode::NoContext>::from_text(&source)?;
        ParsedModule::<StubInst, NoHeader>::parse_section(file.section("main").unwrap())
    }

    fn symbols(parsed: &ParsedModule<StubInst, NoHeader>) -> Vec<(&str, u32)> {
        parsed
            .symbols
            .iter()
            .map(|symbol| (symbol.name.as_str(), symbol.address))
            .collect()
    }

    #[test]
    fn parses_empty_text() {
        let parsed = parse("").unwrap();
        assert!(parsed.code.is_empty());
        assert!(parsed.symbols.is_empty());
    }

    #[test]
    fn symbols_bind_the_next_address_without_occupying_slots() {
        let parsed = parse("@entry:\n@alias:\nstub.halt\nstub.print\n@exit:\n").unwrap();
        assert_eq!(parsed.code, [StubInst::Halt, StubInst::Print]);
        assert_eq!(symbols(&parsed), [("entry", 0), ("alias", 0), ("exit", 2)]);
    }

    #[test]
    fn layout_and_indentation_carry_no_meaning() {
        let flat = parse("@main:\nstub.halt\n@done:\nstub.print\n").unwrap();
        let indented = parse("    @main:\n\t\tstub.halt\n  @done:   stub.print   \n").unwrap();
        let one_line = parse("@main: stub.halt @done: stub.print").unwrap();
        assert_eq!(flat, indented);
        assert_eq!(flat, one_line);
    }

    #[test]
    fn skips_line_comments() {
        let parsed = parse("// leading\n@main: // the entry\n    stub.halt // stop\n").unwrap();
        assert_eq!(parsed.code, [StubInst::Halt]);
        assert_eq!(symbols(&parsed), [("main", 0)]);
    }

    #[test]
    fn rejects_unknown_or_malformed_text() {
        assert!(parse("foo bar 1 2.0").is_err());
        assert!(
            parse("@main").is_err(),
            "a bare reference is not a definition"
        );
        assert!(parse("@: stub.halt").is_err());
        assert!(parse("fn @main() { stub.halt }").is_err());
    }

    #[test]
    fn rejects_malformed_known_instruction() {
        #[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
        #[syntax_class(instruction, head = "stub")]
        enum OnlyOne {
            Dump(u32),
        }

        let file = vihaco_bytecode::SSTFile::<vihaco_bytecode::NoContext>::from_text(
            "sst v2\n.section main\n.text\nstub.dump foo\n",
        )
        .unwrap();
        assert!(
            ParsedModule::<OnlyOne, NoHeader>::parse_section(file.section("main").unwrap())
                .is_err()
        );
    }

    #[test]
    fn lexical_helpers_return_explicit_newtypes() {
        assert_eq!(
            symbol_definition().parse("@main:").into_result(),
            Ok("main".to_owned())
        );
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
