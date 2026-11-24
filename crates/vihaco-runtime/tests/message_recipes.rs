// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[test]
fn cross_crate_message_recipes_and_custom_builders() {
    let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = std::process::Command::new(env!("CARGO"))
        .args(["test", "--offline", "--manifest-path"])
        .arg(package.join("tests/fixtures/messages/Cargo.toml"))
        .arg("--target-dir")
        .arg(package.join("../../target/message-fixtures"))
        .output()
        .expect("run cross-crate message tests");
    assert!(
        output.status.success(),
        "message fixtures failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(feature = "derive")]
#[test]
fn a_recipe_does_not_automatically_supply_a_composite_builder() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/messages/*.rs");
}
