use eyre::Result;

use crate::Effects;

pub trait GeneratedComponent {
    type Instruction;
    type Message;
    type Effect;

    fn execute_generated(
        &mut self,
        inst: Self::Instruction,
        msg: Self::Message,
    ) -> Result<Effects<Self::Effect>>;
}

pub fn expect_exactly_one_effect<E>(effects: Effects<E>) -> Result<E> {
    let mut iter = effects.into_iter();
    let first = iter.next();
    let second = iter.next();
    let effect_count = usize::from(first.is_some()) + usize::from(second.is_some()) + iter.count();
    match (first, second) {
        (Some(effect), None) => Ok(effect),
        _ => Err(eyre::eyre!(
            "expected exactly one effect, got {}",
            effect_count
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeMetadata {
    pub devices: &'static [crate::metadata::DeviceMetadata],
    pub source_symbol_aliases: &'static [crate::metadata::SourceSymbolAliasMetadata],
}

impl CompositeMetadata {
    pub fn devices(&self) -> &'static [crate::metadata::DeviceMetadata] {
        self.devices
    }

    pub fn device_by_name(&self, name: &str) -> Option<&'static crate::metadata::DeviceMetadata> {
        self.devices.iter().find(|device| device.name == name)
    }

    pub fn source_symbol_aliases(&self) -> &'static [crate::metadata::SourceSymbolAliasMetadata] {
        self.source_symbol_aliases
    }

    pub fn source_symbol_device_code(&self, name: &str) -> Option<u8> {
        if let Some(alias) = self
            .source_symbol_aliases
            .iter()
            .find(|alias| alias.name == name)
        {
            return Some(alias.device_code);
        }
        self.device_by_name(name).map(|device| device.code)
    }

    pub fn validate_source_symbols<I, V, Ty, Info>(
        &self,
        module: &crate::module::Module<I, V, Ty, Info>,
    ) -> Result<()> {
        let unresolved: Vec<String> = module
            .source_symbols
            .iter()
            .filter(|symbol| self.source_symbol_device_code(&symbol.name).is_none())
            .map(|symbol| format!("{}: {}", symbol.index, symbol.name))
            .collect();

        if unresolved.is_empty() {
            return Ok(());
        }

        Err(eyre::eyre!(
            "module declares unresolved source symbols for this machine: {}",
            unresolved.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::expect_exactly_one_effect;
    use crate::Effects;

    #[test]
    fn expect_exactly_one_effect_accepts_single_effect() {
        let effect = expect_exactly_one_effect(Effects::one(7)).unwrap();

        assert_eq!(effect, 7);
    }

    #[test]
    fn expect_exactly_one_effect_rejects_zero_effects() {
        let err = expect_exactly_one_effect::<u8>(Effects::none()).unwrap_err();

        assert_eq!(err.to_string(), "expected exactly one effect, got 0");
    }

    #[test]
    fn expect_exactly_one_effect_rejects_multiple_effects() {
        let err = expect_exactly_one_effect(Effects::from(vec![1, 2])).unwrap_err();

        assert_eq!(err.to_string(), "expected exactly one effect, got 2");
    }
}
