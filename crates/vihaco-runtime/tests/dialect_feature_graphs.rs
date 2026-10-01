// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Separate Cargo graphs prove macro discovery with renamed dependencies and
//! keep feature-off checks independent from feature unification in this crate.

#[test]
fn isolated_dialect_dependency_graphs() {
    let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = package.join("tests/fixtures/dialects/Cargo.toml");
    let target = package.join("../../target/dialect-fixtures");
    for packages in [
        &["dialect-feature-off"][..],
        &["dialect-producer", "dialect-consumer"],
        &["dialect-facade"],
        &["dialect-indirect"],
    ] {
        let mut command = std::process::Command::new(env!("CARGO"));
        command
            .args(["test", "--offline", "--manifest-path"])
            .arg(&manifest)
            .arg("--target-dir")
            .arg(&target);
        for package in packages {
            command.args(["-p", package]);
        }
        let output = command.output().expect("run Cargo fixture tests");
        assert!(
            output.status.success(),
            "fixture graph {packages:?} failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
