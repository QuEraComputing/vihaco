use vihaco::dialect;

/// Parses as a number, but neither writes itself back (`Display`) nor has
/// `OperandText`, so instruction text could not show it as source.
#[derive(Clone, Debug, PartialEq)]
pub struct Gain(pub u32);

impl<'src> vihaco::Parse<'src> for Gain {
    fn parser() -> impl vihaco::chumsky::Parser<
        'src,
        &'src str,
        Self,
        vihaco::chumsky::extra::Err<vihaco::chumsky::error::Simple<'src, char>>,
    > {
        use vihaco::chumsky::Parser as _;
        <u32 as vihaco::Parse>::parser().map(Self)
    }
}

#[dialect]
mod amplifier {
    use super::Gain;

    #[instruction]
    #[pattern = "'set_gain $0"]
    pub struct SetGain(pub Gain);
}

fn main() {}
