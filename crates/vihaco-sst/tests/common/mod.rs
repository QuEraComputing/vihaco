// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! A tiny stand-in machine, so the checker can be tested without a
//! feature-dependent `#[composite]`.

use vihaco_abi::Lower;
use vihaco_module::loader::MetadataLoweringContext;
use vihaco_parser::{Ident, QuotedString};
use vihaco_runtime::HasInstructionSet;
use vihaco_sst::{Report, SourceFile};

/// A branch or call target: an address or a `@name` symbol.
#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(value)]
pub enum Target {
    #[pattern = "$0"]
    Resolved(u32),
    #[pattern = "`@` $0"]
    Symbol(Ident),
}

#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(instruction, head = "t")]
pub enum Inst {
    Halt,
    Push(u64),
    #[pattern = "'br $0"]
    Br(Target),
    #[pattern = "'call $0 `,` $1"]
    Call(u32, Target),
    #[pattern = "'print $0"]
    Print(QuotedString),
}

impl Lower<MetadataLoweringContext> for Inst {
    type Runtime = ();

    fn lower(self, context: &mut MetadataLoweringContext) -> eyre::Result<()> {
        match self {
            Inst::Br(Target::Symbol(name)) => context.label_address(name.as_str()).map(drop),
            Inst::Call(_, Target::Symbol(name)) => {
                context.function_address(name.as_str()).map(drop)
            }
            Inst::Print(text) => context.string_index(text.as_str()).map(drop),
            _ => Ok(()),
        }
    }
}

/// The machine whose syntax is [`Inst`].
pub struct Machine;

impl HasInstructionSet for Machine {
    type Runtime = ();
    type Syntax = Inst;
}

/// `line:column: message` for every diagnostic.
pub fn summary(report: &Report) -> Vec<String> {
    report
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.location, diagnostic.message))
        .collect()
}

pub fn source(text: &str) -> SourceFile {
    SourceFile::new("test.sst", text)
}

/// A standard header in `f.sst` declaring parameterless `functions`, with
/// `@main` as the entry.
pub fn header(functions: &[&str]) -> String {
    let mut header = String::from(".file \"f.sst\"\n");
    for function in functions {
        header.push_str(&format!(".func @{function}(), locals 0\n"));
    }
    header.push_str(".entry @main\n");
    header
}
