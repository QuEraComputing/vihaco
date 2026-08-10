// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use eyre::Result;
use vihaco::{
    ContextHandle, Effects, Execute, Execution, LoadOwnSstSection, LoadSstSection, NoEffect,
    NoMessage, ProgramImage, SstFile, SstGlobalContext, SstHeader, SstSectionView, StepResult,
    Type, Value,
    syntax::{Param, ParsedFunction, ParsedModule},
    traits::FromText,
};
use vihaco_parser::Ident;
use vihaco_parser_derive::Parse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Parse)]
#[syntax_class(type)]
enum ParsedType {
    #[pattern = "`i64`"]
    I64,
}

impl From<ParsedType> for Type {
    fn from(_value: ParsedType) -> Self {
        Self::I64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeInstruction;

#[derive(Debug, Default)]
struct TestComponent;

impl Execute<RuntimeInstruction> for TestComponent {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &RuntimeInstruction,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        Ok(StepResult {
            effects: Effects::none(),
            execution: Execution::Complete,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestContext {
    name: String,
}

impl SstGlobalContext for TestContext {
    fn from_text(text: &str) -> Result<Self> {
        Ok(Self {
            name: text.trim().to_owned(),
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct NoHeader;

impl SstHeader for NoHeader {}

impl FromText for NoHeader {
    fn from_text(_text: &str) -> Result<Self> {
        Ok(Self)
    }
}

#[derive(Debug, Default)]
struct ChildLoader {
    loaded_sst: Option<String>,
    context: Option<ContextHandle<TestContext>>,
}

impl LoadSstSection<TestContext> for ChildLoader {
    fn load_sst_section<'src>(&mut self, section: SstSectionView<'src, TestContext>) -> Result<()> {
        self.loaded_sst = Some(section.sst().to_owned());
        self.context = Some(section.context_handle());
        Ok(())
    }
}

vihaco::composite! {
    #[derive(Default)]
    #[allow(dead_code)]
    composite TestMachine {
        error = eyre::Report;

        #[device(0x01)]
        component: TestComponent,

        #[program]
        program: ProgramImage<TestMachineInstruction, TestContext, Value, Type>,

        #[device(0x02)]
        #[loadable("child")]
        child: ChildLoader,
    }

    syntax {
        #[pattern = "'test::run"]
        Run => runtime Run;
    }

    runtime {
        Run(RuntimeInstruction) => component {
            message none;
        }
    }
}

impl LoadOwnSstSection<TestContext> for TestMachine {
    fn load_own_sst_section<'src>(
        &mut self,
        section: SstSectionView<'src, TestContext>,
    ) -> Result<()> {
        let context = section.context_handle();
        let parsed = ParsedModule {
            header: NoHeader,
            functions: vec![ParsedFunction {
                name: Ident("main".to_owned()),
                params: Vec::<Param<ParsedType>>::new(),
                return_ty: None,
                body: vec![test_machine::syntax::Instruction::Run],
            }],
        };
        self.load_parsed(parsed, context)
    }
}

fn root_file(source: &str) -> SstFile<TestContext> {
    SstFile::from_text(&format!(
        "sst v1\n\n.global:\nroot-context\n.global.\n\n{source}"
    ))
    .expect("test SST should parse")
}

#[test]
fn generated_sst_root_loads_program_and_forwards_children() {
    let file = root_file(
        ".section(root):\n\
\t.text(root):\n\
\t\tfn @main() {\n\
\t\t\ttest::run\n\
\t\t}\n\
\t.text(root).\n\
\t.section(child):\n\
\t\t.text(child):\n\
\t\t\tchild payload\n\
\t\t.text(child).\n\
\t.section(child).\n\
.section(root).\n",
    );
    let context = file.context_handle();
    let mut machine = TestMachine::default();

    machine.load_sst_section(file.root()).unwrap();

    assert_eq!(machine.program.module.code.len(), 1);
    assert_eq!(machine.program.module.functions.len(), 1);
    assert_eq!(machine.program.module.main_function, Some(0));
    assert_eq!(machine.program.pc, 0);
    assert!(machine.program.context.is_some());
    assert!(
        machine
            .child
            .loaded_sst
            .as_deref()
            .is_some_and(|sst| sst.contains("child payload"))
    );
    assert!(
        machine
            .child
            .context
            .as_ref()
            .is_some_and(|loaded| loaded.ptr_eq(&context))
    );
}

#[test]
fn malformed_root_source_is_rejected_before_program_installation() {
    let file = root_file(
        ".section(root):\n\
\t.text(root):\n\
\t\tfn @main() {\n\
\t\t\ttest::does_not_exist\n\
\t\t}\n\
\t.text(root).\n\
.section(root).\n",
    );
    let result =
        ParsedModule::<test_machine::syntax::Instruction, ParsedType, NoHeader>::parse_section(
            file.root(),
        );
    let error = match result {
        Ok(_) => panic!("malformed source unexpectedly parsed"),
        Err(error) => error,
    };

    assert!(!error.to_string().is_empty());
    let machine = TestMachine::default();
    assert!(machine.program.module.code.is_empty());
    assert!(machine.program.context.is_none());
    assert_eq!(machine.child.loaded_sst, None);
}
