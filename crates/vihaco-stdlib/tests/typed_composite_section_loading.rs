// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "typed-declarations")]

use framework::composite;
use framework::loader::{
    LoadBytecodeSection, LoadOwnBytecodeSection, LoadOwnSstSection, LoadSstSection,
};

use std::cell::RefCell;
use std::rc::Rc;
use vihaco_bytecode::{
    BytecodeFile, BytecodeGlobalContext, SectionNameResolver, SstFile, SstGlobalContext,
};

use vihaco_runtime as framework;

type Events = Rc<RefCell<Vec<String>>>;

struct Context;

impl SectionNameResolver for Context {
    fn section_name(&self, index: u32) -> Option<&str> {
        match index {
            0 => Some("custom"),
            1 => Some("leaf"),
            2 => Some("unexpected"),
            _ => None,
        }
    }
}

impl BytecodeGlobalContext for Context {
    fn from_bytes(_: &[u8]) -> eyre::Result<Self> {
        Ok(Self)
    }
}

impl SstGlobalContext for Context {
    fn from_text(_: &str) -> eyre::Result<Self> {
        Ok(Self)
    }
}

struct Image(Events);

impl LoadBytecodeSection<Context> for Image {
    fn load_bytecode_section<'bc>(
        &mut self,
        section: framework::BytecodeSectionView<'bc, Context>,
    ) -> eyre::Result<()> {
        self.0.borrow_mut().push(section.display_path().to_string());
        Ok(())
    }
}

impl LoadSstSection<Context> for Image {
    fn load_sst_section<'bc>(
        &mut self,
        section: framework::SstSectionView<'bc, Context>,
    ) -> eyre::Result<()> {
        self.0.borrow_mut().push(section.display_path().to_string());
        Ok(())
    }
}

#[composite]
#[vihaco(crate = framework)]
struct Middle {
    events: Events,
    #[device(3)]
    #[loadable]
    leaf: Image,
}

#[composite]
#[vihaco(crate = framework)]
struct Root {
    events: Events,
    // Ordinary program state retains its authored ProgramCounter behavior.
    program: vihaco_stdlib::ProgramImage<u8, Context>,
    #[device(4, alias = "parser_name")]
    #[loadable("custom")]
    middle: Middle,
}

#[cfg(not(feature = "program-image-execution"))]
impl vihaco_module::host::ProgramCounter for Root {
    type Instruction = u8;
    fn pc(&self) -> u32 {
        self.program.pc
    }
    fn pc_mut(&mut self) -> &mut u32 {
        &mut self.program.pc
    }
    fn get_instruction(&self, pc: u32) -> eyre::Result<&u8> {
        vihaco_module::host::ProgramCounter::get_instruction(&self.program, pc)
    }
}

macro_rules! own {
    ($ty:ty) => {
        impl LoadOwnBytecodeSection<Context> for $ty {
            fn load_own_bytecode_section<'bc>(
                &mut self,
                section: framework::BytecodeSectionView<'bc, Context>,
            ) -> eyre::Result<()> {
                self.events
                    .borrow_mut()
                    .push(section.display_path().to_string());
                Ok(())
            }
        }
        impl LoadOwnSstSection<Context> for $ty {
            fn load_own_sst_section<'bc>(
                &mut self,
                section: framework::SstSectionView<'bc, Context>,
            ) -> eyre::Result<()> {
                self.events
                    .borrow_mut()
                    .push(section.display_path().to_string());
                Ok(())
            }
        }
    };
}

own!(Middle);

own!(Root);

fn section(children: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let table_end = 28 + 12 * children.len();
    let size = table_end + children.iter().map(|(_, bytes)| bytes.len()).sum::<usize>();
    out.extend_from_slice(&(size as u64).to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes()); // header length
    out.extend_from_slice(&0u64.to_le_bytes()); // bytecode length
    out.extend_from_slice(&(children.len() as u32).to_le_bytes());
    let mut offset = table_end;
    for (name, bytes) in children {
        out.extend_from_slice(&name.to_le_bytes());
        out.extend_from_slice(&(offset as u64).to_le_bytes());
        offset += bytes.len();
    }
    for (_, bytes) in children {
        out.extend_from_slice(bytes);
    }
    out
}

fn bytecode(children: &[(u32, Vec<u8>)]) -> BytecodeFile<Context> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(vihaco_bytecode::MAGIC);
    bytes.extend_from_slice(&vihaco_bytecode::VERSION.to_le_bytes());
    bytes.extend_from_slice(&vihaco_bytecode::FLAGS.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes()); // context length
    bytes.extend_from_slice(&section(children));
    BytecodeFile::from_bytes(bytes).unwrap()
}

#[test]
fn typed_composite_section_loading() {
    let events = Events::default();
    let mut root = Root {
        events: events.clone(),
        program: vihaco_stdlib::ProgramImage::default(),
        middle: Middle {
            events: events.clone(),
            leaf: Image(events.clone()),
        },
    };
    assert_eq!(framework::__private::capabilities::<Root>().count(), 0);
    let file = bytecode(&[(0, section(&[(1, section(&[]))]))]);
    root.load_bytecode_section(file.root()).unwrap();
    assert_eq!(*events.borrow(), ["<root>", "custom", "custom/leaf"]);
    events.borrow_mut().clear();
    let text = format!(
        "sst v{}\n.section(root):\n.section(custom):\n.section(leaf):\n.section(leaf).\n.section(custom).\n.section(root).\n",
        vihaco_bytecode::VERSION
    );
    let file = SstFile::<Context>::from_text(&text).unwrap();
    root.load_sst_section(file.root()).unwrap();
    assert_eq!(*events.borrow(), ["<root>", "custom", "custom/leaf"]);
    events.borrow_mut().clear();
    root.load_bytecode_section(bytecode(&[]).root()).unwrap();
    assert_eq!(*events.borrow(), ["<root>"]);
    events.borrow_mut().clear();
    let err = root
        .load_bytecode_section(bytecode(&[(2, section(&[]))]).root())
        .unwrap_err();
    assert!(err.to_string().contains("unexpected child section"));
    assert_eq!(*events.borrow(), ["<root>"]);
    #[cfg(not(feature = "program-image-execution"))]
    {
        root.program.module.code = vec![9];
        use vihaco_module::host::ProgramCounter;
        assert_eq!(*root.get_instruction(root.pc()).unwrap(), 9);
        *root.pc_mut() = 1;
        assert!(root.get_instruction(root.pc()).is_err());
    }
    #[cfg(feature = "program-image-execution")]
    {
        root.program.module.code = vec![9];
        assert_eq!(root.program.module.code, [9]);
    }
}
