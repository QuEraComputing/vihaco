// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser as _;
use eyre::Result;
use vihaco::{
    BuildProgramModule, ContextHandle, Execute, Execution, FromText, InstallProgramModule,
    InstructionSet, ModuleSyntax, NoEffect, NoMessage, Parse, ProgramImage, SstFile,
    SstGlobalContext, SstHeader, SstSectionView, StepResult, Type, Value, composite,
    loader::{LoadSstProgram, LoadSstSubtree},
    module::{FunctionInfo, LabelInfo, LocalModule, SourceSymbolInfo},
    syntax::{ParsedFunction, ParsedModule, Resolve},
};
use vihaco_parser::Ident;

#[derive(Clone, Debug, PartialEq, Parse)]
#[syntax_class(instruction)]
enum LocalInstruction {
    #[pattern = "'local::run"]
    Run,
}

#[derive(Clone, Debug, PartialEq, Parse)]
#[syntax_class(type)]
enum LocalType {
    #[pattern = "`unit`"]
    Unit,
}

#[derive(Clone, Debug, PartialEq, Parse)]
#[syntax_class(value)]
enum LocalValue {
    #[pattern = "`zero`"]
    Zero,
}

struct LocalInstructionSet;

impl InstructionSet for LocalInstructionSet {
    type Instruction = LocalInstruction;
    type Value = LocalValue;
    type Type = LocalType;
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalHeader(String);

impl FromText for LocalHeader {
    fn from_text(text: &str) -> Result<Self> {
        Ok(Self(text.trim().to_owned()))
    }
}

impl SstHeader for LocalHeader {}

struct LocalSyntax;

impl ModuleSyntax for LocalSyntax {
    type Instruction = LocalInstruction;
    type Value = LocalValue;
    type Type = LocalType;
    type Header = LocalHeader;
}

#[derive(Default)]
struct ResolverProbe {
    header: Option<LocalHeader>,
}

impl Resolve<LocalSyntax> for ResolverProbe {
    type Module = ParsedModule<LocalSyntax>;

    fn resolve_module(&mut self, parsed: ParsedModule<LocalSyntax>) -> Result<Self::Module> {
        self.header = Some(parsed.header.clone());
        Ok(parsed)
    }
}

#[test]
fn resolve_receives_the_complete_parsed_module_and_header() {
    let parsed = ParsedModule {
        header: LocalHeader("machine-config".to_owned()),
        functions: vec![ParsedFunction {
            name: Ident("main".to_owned()),
            params: Vec::new(),
            return_ty: Some(LocalType::Unit),
            body: vec![LocalInstruction::Run],
        }],
        labels: Vec::new(),
        constants: Vec::new(),
        strings: Vec::new(),
        source_symbols: Vec::new(),
    };
    let mut resolver = ResolverProbe::default();

    let resolved = resolver.resolve_module(parsed).unwrap();

    assert_eq!(
        resolver.header,
        Some(LocalHeader("machine-config".to_owned()))
    );
    assert_eq!(resolved.functions[0].body, vec![LocalInstruction::Run]);
    assert_eq!(resolved.functions[0].return_ty, Some(LocalType::Unit));
}

#[test]
fn component_instruction_set_is_independent_of_mounting() {
    fn require_instruction_set<T: InstructionSet>() {}
    fn require_surface<T: vihaco::SurfaceInstruction>() {}

    require_instruction_set::<LocalInstructionSet>();
    require_surface::<LocalInstruction>();
    assert_eq!(
        LocalInstruction::parser().parse("local::run").into_result(),
        Ok(LocalInstruction::Run)
    );
    assert_eq!(
        LocalValue::parser().parse("zero").into_result(),
        Ok(LocalValue::Zero)
    );
    assert_eq!(
        LocalType::parser().parse("unit").into_result(),
        Ok(LocalType::Unit)
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeInstruction;

#[derive(Debug, Default)]
struct RuntimeComponent;

impl Execute<RuntimeInstruction> for RuntimeComponent {
    type Message = NoMessage;
    type Effect = NoEffect;
    type Fault = eyre::Report;

    fn execute(
        &mut self,
        _instruction: &RuntimeInstruction,
        _message: Self::Message,
    ) -> Result<StepResult<Self::Effect>, Self::Fault> {
        Ok(StepResult {
            effects: vihaco::Effects::none(),
            execution: Execution::Complete,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestContext(String);

impl SstGlobalContext for TestContext {
    fn from_text(text: &str) -> Result<Self> {
        Ok(Self(text.trim().to_owned()))
    }
}

#[derive(Default)]
struct MinimalProgram {
    module: Option<LocalModule<CustomMachineInstruction, Value, Type>>,
    context: Option<ContextHandle<TestContext>>,
}

impl From<custom_machine::syntax::Type> for Type {
    fn from(value: custom_machine::syntax::Type) -> Self {
        match value {}
    }
}

impl BuildProgramModule for MinimalProgram {
    type Instruction = CustomMachineInstruction;
    type Value = Value;
    type Type = Type;
    type Info = vihaco::module::NoInfo;
    type Module = LocalModule<Self::Instruction, Self::Value, Self::Type>;

    fn empty_module() -> Self::Module {
        LocalModule::default()
    }

    fn append_instructions(
        module: &mut Self::Module,
        instructions: impl IntoIterator<Item = Self::Instruction>,
    ) {
        module.code.extend(instructions);
    }

    fn instruction_count(module: &Self::Module) -> u32 {
        module.code.len() as u32
    }

    fn add_function(module: &mut Self::Module, function: FunctionInfo<Self::Type>) {
        module.functions.push(function);
    }

    fn add_label(module: &mut Self::Module, label: LabelInfo) {
        module.labels.push(label);
    }

    fn add_source_symbol(module: &mut Self::Module, symbol: SourceSymbolInfo) {
        module.source_symbols.push(symbol);
    }

    fn intern_string(module: &mut Self::Module, value: String) -> u32 {
        if let Some(index) = module.strings.iter().position(|item| item == &value) {
            index as u32
        } else {
            let index = module.strings.len() as u32;
            module.strings.push(value);
            index
        }
    }

    fn add_constant(module: &mut Self::Module, value: Self::Value) -> u32 {
        let index = module.constants.len() as u32;
        module.constants.push(value);
        index
    }

    fn set_main_function(module: &mut Self::Module, function: Option<u32>) {
        module.main_function = function;
    }

    fn finish(module: Self::Module) -> Result<Self::Module> {
        Ok(module)
    }
}

impl InstallProgramModule<TestContext> for MinimalProgram {
    type Module = LocalModule<CustomMachineInstruction, Value, Type>;

    fn install_program_module(
        &mut self,
        module: Self::Module,
        context: ContextHandle<TestContext>,
    ) -> Result<()> {
        self.module = Some(module);
        self.context = Some(context);
        Ok(())
    }
}

composite! {
    #[derive(Default)]
    #[allow(dead_code)]
    composite CustomMachine {
        error = eyre::Report;

        #[device(0x01)]
        component: RuntimeComponent,

        #[program]
        program: MinimalProgram,
    }

    syntax {
        header LocalHeader => resolve_header;
        #[pattern = "'custom::run"]
        Run => runtime Run;
    }

    runtime {
        Run(RuntimeInstruction) => component {
            message none;
        }
    }
}

impl custom_machine::syntax::Resolver for CustomMachine {
    fn resolve_header(&mut self, header: LocalHeader) -> Result<(), eyre::Report> {
        if header.0 == "reject" {
            Err(eyre::eyre!("rejected custom header"))
        } else {
            Ok(())
        }
    }
}

impl LoadSstProgram<TestContext> for CustomMachine {
    fn load_sst_program<'src>(&mut self, section: SstSectionView<'src, TestContext>) -> Result<()> {
        self.load_source(section)
    }
}

fn custom_file(header: &str) -> SstFile<TestContext> {
    SstFile::from_text(&format!(
        "sst v1\n\n.global:\ncontext\n.global.\n\n.section(root):\n\t.header(root):\n\t\t{header}\n\t.header(root).\n\t.text(root):\n\t\tfn @main() {{\n\t\t\tcustom::run\n\t\t}}\n\t.text(root).\n.section(root).\n"
    ))
    .expect("test SST should parse")
}

#[test]
fn generated_loader_uses_minimal_builder_and_installs_expanded_program() {
    let file = custom_file("accepted");
    let context = file.context_handle();
    let mut machine = CustomMachine::default();

    machine.load_sst_subtree(file.root()).unwrap();

    let program = machine.program.module.as_ref().unwrap();
    assert_eq!(program.code.len(), 1);
    assert_eq!(program.functions[0].start_address, 0);
    assert_eq!(program.functions[0].end_address, 1);
    assert!(machine.program.context.as_ref().unwrap().ptr_eq(&context));
}

#[test]
fn rejected_composite_header_does_not_install_partial_program() {
    let file = custom_file("reject");
    let mut machine = CustomMachine::default();

    let error = machine.load_sst_subtree(file.root()).unwrap_err();

    assert!(error.to_string().contains("rejected custom header"));
    assert!(machine.program.module.is_none());
    assert!(machine.program.context.is_none());
}

#[test]
fn generated_sums_expose_instruction_value_type_and_alias_contracts() {
    fn require_module_syntax<S: ModuleSyntax>() {}
    require_module_syntax::<custom_machine::syntax::Module>();

    let instruction = custom_machine::syntax::Instruction::parser()
        .parse("custom::run")
        .into_result()
        .unwrap();
    assert!(matches!(
        instruction,
        custom_machine::syntax::Instruction::Run
    ));
}

// Keep the standard implementation in this integration crate's dependency graph so this
// fixture also guards that runtime-only and syntax-bearing components remain composable.
#[allow(dead_code)]
fn require_standard_program_image() {
    fn require_image<T>()
    where
        T: Default,
    {
    }
    require_image::<ProgramImage<CustomMachineInstruction, TestContext>>();
}
