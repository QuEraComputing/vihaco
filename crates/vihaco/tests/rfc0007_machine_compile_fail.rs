#[test]
fn machine_derive_rejects_ambiguous_wiring() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/rfc0007-machine-duplicate-device-code.rs");
    t.compile_fail("tests/ui/rfc0007-machine-duplicate-source-symbol.rs");
    t.compile_fail("tests/ui/rfc0007-machine-shared-unknown-core.rs");
    t.compile_fail("tests/ui/rfc0007-machine-shared-non-core.rs");
    t.compile_fail("tests/ui/rfc0007-machine-scheduler-device-conflict.rs");
    t.compile_fail("tests/ui/rfc0007-machine-ctx-with-unsupported.rs");
}
