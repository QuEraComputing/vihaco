use vihaco_runtime::{composite, loader::{LoadOwnSstSection, LoadSstSection}};

struct Child;
impl LoadSstSection<()> for Child {
    type Output = u32;
    fn load_sst_section<'a>(&mut self, _: vihaco_runtime::SstSectionView<'a, ()>) -> eyre::Result<u32> {
        Ok(7)
    }
}

#[composite]
struct Machine {
    #[device(1)] #[loadable]
    child: Child,
}

impl LoadOwnSstSection<()> for Machine {
    type Output = ();
    fn load_own_sst_section<'a>(&mut self, _: vihaco_runtime::SstSectionView<'a, ()>) -> eyre::Result<()> {
        Ok(())
    }
}

fn load(section: vihaco_runtime::SstSectionView<'_, ()>) {
    let _ = Machine { child: Child }.load_sst_section(section);
}
fn main() {}
