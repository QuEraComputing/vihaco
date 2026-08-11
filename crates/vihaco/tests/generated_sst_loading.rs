// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use eyre::Result;
use vihaco::{
    ContextHandle, Effects, Execute, Execution, LoadSstProgram, LoadSstSubtree, NoEffect,
    NoMessage, ProgramImage, SstFile, SstGlobalContext, SstSectionView, StepResult, Type, Value,
    syntax::{Param, ParsedFunction, ParsedModule},
};
use vihaco_parser::Ident;

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

impl From<test_machine::syntax::Type> for Type {
    fn from(value: test_machine::syntax::Type) -> Self {
        match value {}
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

#[derive(Debug, Default)]
struct ChildLoader {
    loaded_sst: Option<String>,
    context: Option<ContextHandle<TestContext>>,
}

impl LoadSstSubtree<TestContext> for ChildLoader {
    fn load_sst_subtree<'src>(&mut self, section: SstSectionView<'src, TestContext>) -> Result<()> {
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
        #[pattern = "'test::burst $0"]
        Burst(u32) => lower_burst;
    }

    runtime {
        Run(RuntimeInstruction) => component {
            message none;
        },
        Burst(RuntimeInstruction) => component {
            message none;
        }
    }
}

impl test_machine::syntax::Resolver for TestMachine {
    fn lower_burst(
        &mut self,
        count: u32,
    ) -> std::result::Result<Vec<test_machine::runtime::Instruction>, eyre::Report> {
        Ok((0..count)
            .map(|_| TestMachineInstruction::Burst(RuntimeInstruction))
            .collect())
    }
}

impl LoadSstProgram<TestContext> for TestMachine {
    fn load_sst_program<'src>(&mut self, section: SstSectionView<'src, TestContext>) -> Result<()> {
        self.load_source(section)
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

    machine.load_sst_subtree(file.root()).unwrap();

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
fn generated_load_parsed_installs_the_complete_module_dialect() {
    let file = root_file(
        ".section(root):\n\
\t.text(root):\n\
\t.text(root).\n\
.section(root).\n",
    );
    let context = file.context_handle();
    let mut machine = TestMachine::default();
    let parsed = ParsedModule {
        header: test_machine::syntax::Header,
        functions: vec![ParsedFunction {
            name: Ident("main".to_owned()),
            params: Vec::<Param<test_machine::syntax::Module>>::new(),
            return_ty: None,
            body: vec![test_machine::syntax::Instruction::Run],
        }],
    };

    machine.load_parsed(parsed, context).unwrap();

    assert_eq!(machine.program.module.code.len(), 1);
    assert!(machine.program.context.is_some());
}

#[test]
fn generated_load_source_expands_one_surface_instruction_to_many() {
    let file = root_file(
        ".section(root):\n\
\t.text(root):\n\
\t\tfn @main() {\n\
\t\t\ttest::burst 3\n\
\t\t}\n\
\t.text(root).\n\
.section(root).\n",
    );
    let mut machine = TestMachine::default();

    machine.load_source(file.root()).unwrap();

    assert_eq!(machine.program.module.code.len(), 3);
    assert_eq!(machine.program.module.functions[0].start_address, 0);
    assert_eq!(machine.program.module.functions[0].end_address, 3);
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
    let mut machine = TestMachine::default();
    let error = machine.load_source(file.root()).unwrap_err();

    assert!(!error.to_string().is_empty());
    assert!(machine.program.module.code.is_empty());
    assert!(machine.program.context.is_none());
    assert_eq!(machine.child.loaded_sst, None);
}
