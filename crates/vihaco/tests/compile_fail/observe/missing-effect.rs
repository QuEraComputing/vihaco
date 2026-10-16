use vihaco::observe;

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
    ) -> Result<vihaco::Effects<()>, std::convert::Infallible> {
        let _ = effect.0;
        Ok(vihaco::Effects::none())
    }

    fn observe_another_effect(
        &mut self,
        effect: &AnotherEffect,
    ) -> Result<vihaco::Effects<()>, std::convert::Infallible> {
        let _ = &effect.0;
        Ok(vihaco::Effects::none())
    }
}

fn main() {}
