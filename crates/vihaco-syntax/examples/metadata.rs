// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Run with `cargo run -p vihaco-syntax --example metadata`.

use chumsky::prelude::*;
use vihaco_abi::{Lower, Type, Value, traits::FromText};
use vihaco_bytecode::{NoContext, SstFile, SstHeader};
use vihaco_module::{
    loader::{MetadataLoweringContext, ProgramImage},
    module::ProgramMetadata,
};
use vihaco_parser::Parse as _;
use vihaco_syntax::{ParsedModule, skip};

#[derive(Debug, vihaco_parser_derive::Parse)]
#[syntax_class(instruction, head = "example")]
enum Instruction {
    Load(u32),
    Ret,
}

impl<C> Lower<C> for Instruction {
    type Runtime = Self;

    fn lower(self, _: &mut C) -> eyre::Result<Self> {
        Ok(self)
    }
}

#[derive(Debug, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, open = "{", close = "}")]
struct ExperimentInfo {
    description: String,
}

type Metadata = ProgramMetadata<Value, Type, ExperimentInfo>;

// Existing device-header patterns are reused unchanged.
#[derive(Debug, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, head = "device")]
enum DeviceHeader {
    #[pattern = "circuit.n_qubits $0"]
    NumQubits(usize),
    #[pattern = "circuit.coefficient_threshold $0"]
    CoefficientThreshold(f64),
}

#[derive(vihaco_parser_derive::Parse)]
#[syntax_class(metadata)]
enum Declaration {
    #[pattern = "$0"]
    Metadata(Metadata),
    #[pattern = "$0"]
    Device(DeviceHeader),
}

// The section owner decides which declarations belong in its header and how
// to apply them. Vihaco doesn't impose a universal device-header container.
struct Header {
    metadata: Metadata,
    devices: Vec<DeviceHeader>,
}

impl FromText for Header {
    fn from_text(text: &str) -> eyre::Result<Self> {
        let declarations = Declaration::parser()
            .padded_by(skip())
            .then_ignore(just(';').padded_by(skip()))
            .repeated()
            .collect::<Vec<_>>()
            .then_ignore(skip())
            .then_ignore(end())
            .parse(text)
            .into_result()
            .map_err(|errors| eyre::eyre!("invalid example header: {errors:?}"))?;
        let mut metadata = None;
        let mut devices = Vec::new();
        for declaration in declarations {
            match declaration {
                Declaration::Metadata(value) => {
                    eyre::ensure!(metadata.is_none(), "duplicate metadata declaration");
                    metadata = Some(value);
                }
                Declaration::Device(value) => devices.push(value),
            }
        }
        Ok(Self {
            metadata: metadata.ok_or_else(|| eyre::eyre!("missing required metadata"))?,
            devices,
        })
    }
}

impl SstHeader for Header {}

fn main() -> eyre::Result<()> {
    let file = SstFile::<NoContext>::from_text(include_str!("metadata.sst"))?;
    let section = file.root();
    let context = Some(section.context_handle());
    let ParsedModule { header, functions } =
        ParsedModule::<Instruction, Type, Header>::parse_section(section)?;
    for device in header.devices {
        match device {
            DeviceHeader::NumQubits(count) => println!("Circuit qubits: {count}"),
            DeviceHeader::CoefficientThreshold(threshold) => {
                println!("Coefficient threshold: {threshold}")
            }
        }
    }
    let mut lowering = MetadataLoweringContext::new(&header.metadata)?;
    let module = ParsedModule {
        header: (),
        functions,
    }
    .lower_with_metadata(header.metadata, &mut lowering)?;
    let image = ProgramImage {
        module,
        context,
        pc: 0,
    };
    println!("{:#?}", image.module);
    println!("Description: {}", image.module.extra.description);
    for instruction in &image.module.code {
        if let Instruction::Load(index) = instruction {
            println!("Load references constant {index}");
        }
    }
    Ok(())
}
