// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use eyre::Result;
use vihaco::{
    ContextHandle, Effects, Execute, Execution, LoadSstProgram, LoadSstSubtree, NoEffect,
    NoMessage, ProgramImage, SstFile, SstGlobalContext, SstSectionView, StepResult, Type, Value,
    syntax::{Param, ParsedFunction, ParsedLabel, ParsedModule, ParsedSourceSymbol},
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
        #[pattern = "'test::fail $0"]
        Fail(u32) => lower_fail;
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

    fn lower_fail(
        &mut self,
        count: u32,
    ) -> std::result::Result<Vec<test_machine::runtime::Instruction>, eyre::Report> {
        Err(eyre::eyre!("cannot lower test value {count}"))
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
        labels: vec![ParsedLabel {
            name: Ident("entry".to_owned()),
            function: Ident("main".to_owned()),
            instruction: 0,
        }],
        constants: vec![Value::I64(7)],
        strings: vec!["extra-string".to_owned()],
        source_symbols: vec![ParsedSourceSymbol {
            name: Ident("source-entry".to_owned()),
            index: 4,
        }],
    };

    machine.load_parsed(parsed, context).unwrap();

    assert_eq!(machine.program.module.code.len(), 1);
    assert!(machine.program.context.is_some());
    assert_eq!(machine.program.module.labels[0].address, 0);
    assert_eq!(machine.program.module.constants, vec![Value::I64(7)]);
    assert!(
        machine
            .program
            .module
            .strings
            .contains(&"extra-string".to_owned())
    );
    assert_eq!(machine.program.module.source_symbols[0].index, 4);
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
\t.section(child):\n\
\t\t.text(child):\n\
\t\t\tchild payload\n\
\t\t.text(child).\n\
\t.section(child).\n\
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

mod nested_loading {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ChildRuntimeInstruction;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ParentRuntimeInstruction;

    impl Execute<ChildRuntimeInstruction> for TestComponent {
        type Message = NoMessage;
        type Effect = NoEffect;
        type Fault = eyre::Report;

        fn execute(
            &mut self,
            _instruction: &ChildRuntimeInstruction,
            _message: Self::Message,
        ) -> Result<StepResult<Self::Effect>, Self::Fault> {
            Ok(StepResult {
                effects: Effects::none(),
                execution: Execution::Complete,
            })
        }
    }

    impl Execute<ParentRuntimeInstruction> for TestComponent {
        type Message = NoMessage;
        type Effect = NoEffect;
        type Fault = eyre::Report;

        fn execute(
            &mut self,
            _instruction: &ParentRuntimeInstruction,
            _message: Self::Message,
        ) -> Result<StepResult<Self::Effect>, Self::Fault> {
            Ok(StepResult {
                effects: Effects::none(),
                execution: Execution::Complete,
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ChildHeader;

    impl vihaco::FromText for ChildHeader {
        fn from_text(text: &str) -> Result<Self> {
            (text.trim() == "child-header")
                .then_some(Self)
                .ok_or_else(|| eyre::eyre!("expected child-header"))
        }
    }

    impl vihaco::SstHeader for ChildHeader {}

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ParentHeader;

    impl vihaco::FromText for ParentHeader {
        fn from_text(text: &str) -> Result<Self> {
            (text.trim() == "parent-header")
                .then_some(Self)
                .ok_or_else(|| eyre::eyre!("expected parent-header"))
        }
    }

    impl vihaco::SstHeader for ParentHeader {}

    mod child_def {
        use super::*;

        vihaco::composite! {
            #[derive(Default)]
            #[allow(dead_code)]
        pub composite ChildMachine {
                error = eyre::Report;

                #[device(0x11)]
                component: TestComponent,

                #[program]
            pub program: ProgramImage<ChildMachineInstruction, TestContext, Value, Type>,
            }

            syntax {
                header ChildHeader => resolve_header;
                #[pattern = "'child::run"]
                Run => runtime Run;
            }

            runtime {
                Run(ChildRuntimeInstruction) => component {
                    message none;
                }
            }
        }

        impl From<child_machine::syntax::Type> for Type {
            fn from(value: child_machine::syntax::Type) -> Self {
                match value {}
            }
        }

        impl child_machine::syntax::Resolver for ChildMachine {
            fn resolve_header(&mut self, _header: ChildHeader) -> Result<(), eyre::Report> {
                Ok(())
            }
        }

        impl LoadSstProgram<TestContext> for ChildMachine {
            fn load_sst_program<'src>(
                &mut self,
                section: SstSectionView<'src, TestContext>,
            ) -> Result<()> {
                self.load_source(section)
            }
        }
    }

    mod parent_def {
        use super::child_def::ChildMachine;
        use super::*;

        vihaco::composite! {
            #[derive(Default)]
            #[allow(dead_code)]
        pub composite ParentMachine {
                error = eyre::Report;

                #[device(0x21)]
                component: TestComponent,

                #[program]
            pub program: ProgramImage<ParentMachineInstruction, TestContext, Value, Type>,

                #[device(0x22)]
                #[loadable("child")]
            pub child: ChildMachine,
            }

            syntax {
                header ParentHeader => resolve_header;
                #[pattern = "'parent::run"]
                Run => runtime Run;
            }

            runtime {
                Run(ParentRuntimeInstruction) => component {
                    message none;
                }
            }
        }

        impl From<parent_machine::syntax::Type> for Type {
            fn from(value: parent_machine::syntax::Type) -> Self {
                match value {}
            }
        }

        impl parent_machine::syntax::Resolver for ParentMachine {
            fn resolve_header(&mut self, _header: ParentHeader) -> Result<(), eyre::Report> {
                Ok(())
            }
        }

        impl LoadSstProgram<TestContext> for ParentMachine {
            fn load_sst_program<'src>(
                &mut self,
                section: SstSectionView<'src, TestContext>,
            ) -> Result<()> {
                self.load_source(section)
            }
        }
    }

    fn nested_file(child_header: &str) -> SstFile<TestContext> {
        root_file(&format!(
            ".section(root):\n\t.header(root):\n\t\tparent-header\n\t.header(root).\n\
\t.text(root):\n\
\t\tfn @main() {{\n\
\t\t\tparent::run\n\
\t\t}}\n\
\t.text(root).\n\
\t.section(child):\n\
\t\t.header(child):\n\t\t\t{child_header}\n\t\t.header(child).\n\
\t\t.text(child):\n\
\t\t\tfn @main() {{\n\
\t\t\t\tchild::run\n\
\t\t\t}}\n\
\t\t.text(child).\n\
\t.section(child).\n\
.section(root).\n"
        ))
    }

    #[test]
    fn nested_composites_load_independent_dialects_after_parent_acceptance() {
        let file = nested_file("child-header");
        let mut machine = parent_def::ParentMachine::default();

        machine.load_sst_subtree(file.root()).unwrap();

        assert_eq!(machine.program.module.code.len(), 1);
        assert_eq!(machine.child.program.module.code.len(), 1);
    }

    #[test]
    fn nested_child_failure_does_not_install_parent_program() {
        let file = nested_file("wrong-child-header");
        let mut machine = parent_def::ParentMachine::default();

        let error = machine.load_sst_subtree(file.root()).unwrap_err();

        assert!(error.to_string().contains("child"));
        assert!(machine.program.module.code.is_empty());
        assert!(machine.program.context.is_none());
    }
}

#[test]
fn lowerer_errors_identify_function_instruction_and_surface_value() {
    let file = root_file(
        ".section(root):\n\
\t.text(root):\n\
\t\tfn @entry() {\n\
\t\t\ttest::fail 17\n\
\t\t}\n\
\t.text(root).\n\
.section(root).\n",
    );
    let mut machine = TestMachine::default();

    let error = machine.load_source(file.root()).unwrap_err().to_string();

    assert!(error.contains("function `entry`"), "{error}");
    assert!(error.contains("instruction 0"), "{error}");
    assert!(error.contains("Fail(17)"), "{error}");
    assert!(error.contains("cannot lower test value 17"), "{error}");
    assert!(machine.program.module.code.is_empty());
    assert!(machine.program.context.is_none());
}
