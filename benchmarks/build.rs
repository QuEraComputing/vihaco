// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Generate a stable registry for the native implementations in workload bundles.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo:rerun-if-changed=workloads");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let bundles = bundle_paths(&root.join("workloads"));
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("output directory"));
    fs::write(output.join("native.rs"), registry_source(&bundles))
        .expect("write native workload registry");
}

fn bundle_paths(root: &Path) -> Vec<PathBuf> {
    let mut bundles: Vec<_> = fs::read_dir(root)
        .expect("workloads directory")
        .map(|entry| entry.expect("workload entry").path())
        .filter(|path| path.is_dir())
        .collect();
    bundles.sort();
    bundles
}

fn workload_id(bundle: &Path) -> String {
    let manifest: toml::Value = fs::read_to_string(bundle.join("workload.toml"))
        .expect("workload manifest")
        .parse()
        .expect("valid TOML");
    let id = manifest["id"].as_str().expect("workload id");
    for file in ["native.rs", "python.py", "program.sst"] {
        assert!(bundle.join(file).is_file(), "missing {file} in {id}");
    }
    id.to_owned()
}

fn registry_source(bundles: &[PathBuf]) -> String {
    let mut generated = String::new();
    let mut entries = Vec::new();
    for (index, bundle) in bundles.iter().enumerate() {
        let id = workload_id(bundle);
        let native = bundle.join("native.rs");
        // Numeric module names avoid interpreting manifest IDs as Rust syntax.
        // Debug formatting quotes and escapes source paths and workload IDs.
        generated.push_str(&format!("#[path = {native:?}] mod workload_{index};\n"));
        entries.push(format!(
            "({id:?}, workload_{index}::run as fn(u64, u64) -> u64)"
        ));
    }
    generated.push_str(&format!(
        "pub(super) const NATIVE: &[(&str, Native)] = &[{}];\n",
        entries.join(",")
    ));
    generated
}
