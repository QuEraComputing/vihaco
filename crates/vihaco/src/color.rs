use colored::{Color::TrueColor, Colorize};
use std::fmt::{Display, Formatter, Result as FmtResult};

pub struct Decorated<'a, T: Display> {
    data: &'a T,
    prefix: Option<String>,
    color: Option<colored::Color>,
}

impl<'a, T: Display> Decorated<'a, T> {
    pub fn new(data: &'a T) -> Self {
        Decorated {
            data,
            prefix: None,
            color: None,
        }
    }

    pub fn true_color(&mut self, r: u8, g: u8, b: u8) -> Self {
        Decorated {
            data: self.data,
            prefix: self.prefix.clone(),
            color: Some(TrueColor { r, g, b }),
        }
    }
}

impl<'a, T: Display> Display for Decorated<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let prefix = self.prefix.clone().unwrap_or_default();

        if f.alternate() {
            let formatted = format!("{}{:#}", prefix, self.data);
            if let Some(color) = self.color {
                write!(f, "{}", formatted.color(color))
            } else {
                write!(f, "{}", formatted)
            }
        } else {
            write!(f, "{}", prefix)?;
            self.data.fmt(f)
        }
    }
}

pub trait Themed: Display + Sized {
    fn ty(&self) -> Decorated<'_, Self> {
        Decorated::new(self).true_color(102, 217, 239)
    }
    fn target(&self) -> Decorated<'_, Self> {
        Decorated::new(self).true_color(174, 129, 255)
    }
    fn keyword(&self) -> Decorated<'_, Self> {
        Decorated::new(self).true_color(249, 38, 114)
    }
    fn symbol(&self) -> Decorated<'_, Self> {
        Decorated::new(self).true_color(166, 226, 46)
    }
    fn at_symbol(&self) -> Decorated<'_, Self> {
        let mut decorated = self.symbol();
        decorated.prefix = Some("@".to_string());
        decorated
    }

    fn comment(&self) -> Decorated<'_, Self> {
        Decorated::new(self).true_color(117, 113, 94)
    }

    fn instruction(&self) -> Decorated<'_, Self> {
        Decorated::new(self).true_color(102, 217, 239)
    }
}

impl<T: Display> Themed for T {}

impl<'a, T: Display> From<&'a T> for Decorated<'a, T> {
    fn from(value: &'a T) -> Self {
        Decorated::new(value)
    }
}

impl<'a, T: Display> From<&'a mut T> for Decorated<'a, T> {
    fn from(value: &'a mut T) -> Self {
        Decorated::new(value)
    }
}

impl<T: Display> From<T> for Decorated<'_, T> {
    fn from(value: T) -> Self {
        Decorated::new(Box::leak(Box::new(value)))
    }
}

#[macro_export]
macro_rules! show {
    ($fmt:expr $(, $arg:expr)* $(,)?) => {
        {
            $($arg.fmt($fmt)?;)*
        }
    };
}

#[macro_export]
macro_rules! show_instruction {
    ($fmt:expr, $inst:expr $(, $arg:expr)* $(,)?) => {
        {
            $inst.instruction().fmt($fmt)?;
            $($arg.fmt($fmt)?;)*
        }
    };
}

pub use show;
pub use show_instruction;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decorated() {
        let value = 42;
        let decorated = Decorated::new(&value).true_color(255, 0, 0);
        println!("{}", decorated);
        println!("{:#}", decorated);
    }

    #[test]
    fn test_themed() {
        let value = "test";
        println!("{:#}", value.ty());
        println!("{:#}", value.target());
        println!("{:#}", value.keyword());
        println!("{:#}", value.symbol());
        println!("{:#}", value.at_symbol());
        println!("{:#}", value.comment());
        println!("{:#}", value.instruction());
    }
}
