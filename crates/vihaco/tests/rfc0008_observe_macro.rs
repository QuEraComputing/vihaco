// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use vihaco::{Effects, Observe, observe};

#[derive(Debug, Clone)]
struct TestEffect(pub i32);

#[derive(Default)]
struct TestObserver {
    received: Vec<i32>,
}

#[observe(TestEffect, effect = ())]
impl TestObserver {
    fn observe_test_effect(&mut self, effect: &TestEffect) -> eyre::Result<Effects<()>> {
        self.received.push(effect.0);
        Ok(Effects::none())
    }
}

#[test]
fn observe_macro_generates_trait_impl() {
    let mut obs = TestObserver::default();
    let follow_ups = Observe::<TestEffect>::observe(&mut obs, &TestEffect(42)).unwrap();
    assert_eq!(obs.received, vec![42]);
    assert!(follow_ups.into_iter().next().is_none());
}

#[derive(Debug, Clone)]
struct AnotherEffect(pub String);

#[derive(Default)]
struct MultiObserver {
    ints: Vec<i32>,
    strings: Vec<String>,
}

#[observe(TestEffect, AnotherEffect, effect = ())]
impl MultiObserver {
    fn observe_test_effect(&mut self, effect: &TestEffect) -> eyre::Result<Effects<()>> {
        self.ints.push(effect.0);
        Ok(Effects::none())
    }
    fn observe_another_effect(&mut self, effect: &AnotherEffect) -> eyre::Result<Effects<()>> {
        self.strings.push(effect.0.clone());
        Ok(Effects::none())
    }
}

#[test]
fn observe_macro_handles_multiple_effect_types() {
    let mut obs = MultiObserver::default();
    assert!(
        Observe::<TestEffect>::observe(&mut obs, &TestEffect(1))
            .unwrap()
            .into_iter()
            .next()
            .is_none()
    );
    assert!(
        Observe::<AnotherEffect>::observe(&mut obs, &AnotherEffect("hello".to_string()))
            .unwrap()
            .into_iter()
            .next()
            .is_none()
    );
    assert_eq!(obs.ints, vec![1]);
    assert_eq!(obs.strings, vec!["hello"]);
}

#[derive(Default)]
struct ManualObserver {
    sum: i32,
}

impl Observe<TestEffect> for ManualObserver {
    type Effect = ();
    type Error = eyre::Report;

    fn observe(&mut self, effect: &TestEffect) -> Result<Effects<Self::Effect>, Self::Error> {
        self.sum = effect.0;
        Ok(Effects::none())
    }
}

#[test]
fn manual_observe_impl_uses_result_effects_signature() {
    let mut obs = ManualObserver::default();
    let effects = Observe::<TestEffect>::observe(&mut obs, &TestEffect(42)).unwrap();
    assert!(effects.into_iter().next().is_none());
    assert_eq!(obs.sum, 42);
}

#[derive(Default)]
struct FollowUpObserver {
    seen: Vec<i32>,
}

#[observe(TestEffect, effect = AnotherEffect)]
impl FollowUpObserver {
    fn observe_test_effect(&mut self, effect: &TestEffect) -> eyre::Result<Effects<AnotherEffect>> {
        self.seen.push(effect.0);
        Ok(Effects::one(AnotherEffect(format!("echo:{:?}", effect.0))))
    }
}

#[test]
fn observe_macro_uses_explicit_effect_type_for_follow_up_effects() {
    let mut obs = FollowUpObserver::default();
    let follow_ups = Observe::<TestEffect>::observe(&mut obs, &TestEffect(7)).unwrap();
    assert_eq!(obs.seen, vec![7]);
    let effect = follow_ups.into_iter().next().unwrap();
    assert_eq!(effect.0, "echo:7");
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CompositeEffect {
    Int(i32),
    Text(String),
}

impl From<TestEffect> for CompositeEffect {
    fn from(value: TestEffect) -> Self {
        Self::Int(value.0)
    }
}

impl From<AnotherEffect> for CompositeEffect {
    fn from(value: AnotherEffect) -> Self {
        Self::Text(value.0)
    }
}

#[derive(Default)]
struct ExplicitCompositeObserver {
    seen: Vec<i32>,
}

#[observe(TestEffect, effect = CompositeEffect)]
impl ExplicitCompositeObserver {
    fn observe_test_effect_first(
        &mut self,
        effect: &TestEffect,
    ) -> eyre::Result<Effects<TestEffect>> {
        self.seen.push(effect.0);
        Ok(Effects::one(TestEffect(effect.0 + 1)))
    }

    fn observe_test_effect_second(
        &mut self,
        effect: &TestEffect,
    ) -> eyre::Result<Effects<AnotherEffect>> {
        self.seen.push(effect.0 * 10);
        Ok(Effects::one(AnotherEffect(format!("echo:{}", effect.0))))
    }
}

#[test]
fn observe_macro_lifts_local_effects_into_explicit_composite_effect() {
    let mut obs = ExplicitCompositeObserver::default();
    let follow_ups = Observe::<TestEffect>::observe(&mut obs, &TestEffect(7)).unwrap();

    assert_eq!(obs.seen, vec![7, 70]);
    assert_eq!(
        follow_ups.into_iter().collect::<Vec<_>>(),
        vec![
            CompositeEffect::Int(8),
            CompositeEffect::Text("echo:7".to_string()),
        ]
    );
}

#[derive(Default)]
struct MultiEventExplicitObserver {
    ints: Vec<i32>,
    strings: Vec<String>,
}

#[observe(TestEffect, AnotherEffect, effect = CompositeEffect)]
impl MultiEventExplicitObserver {
    fn observe_test_effect(
        &mut self,
        effect: &TestEffect,
    ) -> eyre::Result<Effects<CompositeEffect>> {
        self.ints.push(effect.0);
        Ok(Effects::one(CompositeEffect::Int(effect.0)))
    }

    fn observe_another_effect(
        &mut self,
        effect: &AnotherEffect,
    ) -> eyre::Result<Effects<CompositeEffect>> {
        self.strings.push(effect.0.clone());
        Ok(Effects::one(CompositeEffect::Text(effect.0.clone())))
    }
}

#[test]
fn observe_macro_supports_explicit_composite_effect_type() {
    let mut obs = MultiEventExplicitObserver::default();

    assert_eq!(
        Observe::<TestEffect>::observe(&mut obs, &TestEffect(3))
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>(),
        vec![CompositeEffect::Int(3)]
    );
    assert_eq!(
        Observe::<AnotherEffect>::observe(&mut obs, &AnotherEffect("hello".to_string()))
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>(),
        vec![CompositeEffect::Text("hello".to_string())]
    );

    assert_eq!(obs.ints, vec![3]);
    assert_eq!(obs.strings, vec!["hello"]);
}

fn write_temp_file(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn temp_crate_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "vihaco-observe-macro-{}-{}-{}",
        name,
        std::process::id(),
        unique
    ))
}

struct TempCrateDir(PathBuf);

impl TempCrateDir {
    fn new(name: &str) -> Self {
        Self(temp_crate_dir(name))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempCrateDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn observe_macro_requires_explicit_effect_for_composite_observers() {
    let dir = TempCrateDir::new("missing-effect");
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let vihaco_path = manifest_dir.canonicalize().unwrap();

    write_temp_file(
        &dir.path().join("Cargo.toml"),
        &format!(
            "[package]\nname = \"missing-effect\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nvihaco = {{ path = {:?} }}\n",
            vihaco_path
        ),
    );
    write_temp_file(
        &dir.path().join("src/lib.rs"),
        r#"
use vihaco::{Effects, observe};

#[derive(Clone)]
struct TestEffect(i32);

#[derive(Clone)]
struct AnotherEffect(String);

#[derive(Default)]
struct MissingEffectObserver;

#[observe(TestEffect, AnotherEffect)]
impl MissingEffectObserver {
    fn observe_test_effect(
        &mut self,
        effect: &TestEffect,
    ) -> std::result::Result<Effects<()>, std::convert::Infallible> {
        let _ = effect.0;
        Ok(Effects::none())
    }

    fn observe_another_effect(
        &mut self,
        effect: &AnotherEffect,
    ) -> std::result::Result<Effects<()>, std::convert::Infallible> {
        let _ = &effect.0;
        Ok(Effects::none())
    }
}
"#,
    );

    let output = Command::new("cargo")
        .arg("check")
        .arg("--offline")
        .arg("--manifest-path")
        .arg(dir.path().join("Cargo.toml"))
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(!output.status.success(), "expected compile failure");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "generated #[observe] impls that compose multiple observed events, multiple handlers, or typed follow-up effects must declare `effect = ...`"
        ),
        "unexpected stderr:\n{stderr}"
    );
}
