// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Bundle contracts, source loading, and pre-measurement correctness checks.

use eyre::{Result, WrapErr, ensure, eyre};
use serde::Deserialize;
use std::{collections::BTreeSet, fs, path::Path};

use crate::machine;

/// Native reference implementations share the same inputs and result type.
pub type Native = fn(u64, u64) -> u64;

// The build script discovers reference implementations without a handwritten list.
include!(concat!(env!("OUT_DIR"), "/native.rs"));

/// One shared input and independently specified expected result.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub iterations: u64,
    pub seed: u64,
    pub expected: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_iterations: u64,
    pub max_seed: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub id: String,
    pub algorithm: String,
    pub numeric_constraints: String,
    pub limits: Limits,
    pub cases: Vec<Case>,
}

/// A validated bundle with its native entry point and resolved SST program.
pub struct Workload {
    pub contract: Contract,
    pub native: Native,
    pub program: machine::Program,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

impl Contract {
    fn validate(&self) -> Result<()> {
        ensure!(valid_id(&self.id), "invalid workload id: {}", self.id);
        ensure!(
            !self.algorithm.is_empty() && !self.numeric_constraints.is_empty(),
            "missing workload contract"
        );
        ensure!(
            self.cases.len() >= 2,
            "workloads need multiple validation cases"
        );
        let mut ids = BTreeSet::new();
        ensure!(
            self.limits.max_iterations <= 1_000_000 && self.limits.max_seed < 65_536,
            "invalid workload limits"
        );
        for case in &self.cases {
            ensure!(
                valid_id(&case.id) && ids.insert(&case.id),
                "invalid or duplicate case id"
            );
            ensure!(
                case.iterations <= self.limits.max_iterations && case.seed <= self.limits.max_seed,
                "case exceeds bounded integer contract"
            );
        }
        Ok(())
    }
}

impl Workload {
    fn load(path: &Path) -> Result<Self> {
        let contract: Contract = toml::from_str(&fs::read_to_string(path.join("workload.toml"))?)?;
        contract.validate()?;
        for file in ["native.rs", "python.py", "program.sst"] {
            ensure!(path.join(file).is_file(), "missing {file}");
        }
        let native = NATIVE
            .iter()
            .find(|(id, _)| *id == contract.id)
            .ok_or_else(|| eyre!("native workload not compiled: {}", contract.id))?
            .1;
        let program = machine::Program::parse(&fs::read_to_string(path.join("program.sst"))?)?;
        Ok(Self {
            contract,
            native,
            program,
        })
    }
}

/// Discover bundles and resolve their SST before any measurements.
pub fn discover(root: &Path) -> Result<Vec<Workload>> {
    let mut paths = fs::read_dir(root)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    let mut ids = BTreeSet::new();
    let mut workloads = Vec::new();
    for path in paths.into_iter().filter(|path| path.is_dir()) {
        let workload = Workload::load(&path)
            .wrap_err_with(|| format!("load workload bundle {}", path.display()))?;
        ensure!(
            ids.insert(workload.contract.id.clone()),
            "duplicate workload id: {}",
            workload.contract.id
        );
        workloads.push(workload);
    }
    ensure!(!workloads.is_empty(), "no workload bundles");
    Ok(workloads)
}

/// Validate native and both VM routes against independent expected results.
pub fn validate(workloads: &[Workload]) -> Result<()> {
    for workload in workloads {
        for case in &workload.contract.cases {
            ensure!(
                (workload.native)(case.iterations, case.seed) == case.expected,
                "native mismatch: {}/{}",
                workload.contract.id,
                case.id
            );
            for composite in [false, true] {
                let mut machine =
                    machine::Fixture::for_program(&workload.program, case.iterations, case.seed);
                let actual = if composite {
                    machine.run::<true>(&workload.program)?
                } else {
                    machine.run::<false>(&workload.program)?
                };
                ensure!(
                    actual == case.expected,
                    "SST mismatch: {}/{} composite={composite}: {actual} != {}",
                    workload.contract.id,
                    case.id,
                    case.expected
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> Contract {
        toml::from_str(include_str!("../workloads/arithmetic/workload.toml")).unwrap()
    }

    #[test]
    fn contract_enforces_workload_specific_limits() {
        let mut contract: Contract = toml::from_str(include_str!(
            "../workloads/recursive_fibonacci/workload.toml"
        ))
        .unwrap();
        contract.validate().unwrap();
        contract.cases[0].iterations = 21;
        assert!(contract.validate().is_err());
        contract.cases[0].iterations = 0;
        contract.limits.max_seed = 0;
        assert!(contract.validate().is_err());
        contract.limits.max_iterations = 1_000_001;
        assert!(contract.validate().is_err());
    }

    #[test]
    fn contract_rejects_invalid_ids_and_duplicate_cases() {
        let mut invalid = contract();
        invalid.id = "not a valid id".into();
        assert!(invalid.validate().is_err());

        let mut duplicate = contract();
        duplicate.cases[1].id = duplicate.cases[0].id.clone();
        assert!(duplicate.validate().is_err());
    }

    #[test]
    fn contract_requires_multiple_bounded_cases() {
        let mut too_few = contract();
        too_few.cases.truncate(1);
        assert!(too_few.validate().is_err());

        let mut too_many_iterations = contract();
        too_many_iterations.cases[0].iterations = 1_000_001;
        assert!(too_many_iterations.validate().is_err());

        let mut large_seed = contract();
        large_seed.cases[0].seed = 65_536;
        assert!(large_seed.validate().is_err());
    }

    #[test]
    fn all_bundles_match_independent_results() {
        let workloads =
            super::discover(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("workloads"))
                .unwrap();
        super::validate(&workloads).unwrap();
    }
}
