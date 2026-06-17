#[test]
fn machine_derive_rejects_ambiguous_wiring() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/rfc0007-machine-duplicate-device-code.rs");
}
