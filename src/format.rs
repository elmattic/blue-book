use rt_format::argument::{FormatArgument, NamedArguments, NoPositionalArguments};
use rt_format::{Format, ParsedFormat, Specifier};

use std::cmp::PartialEq;
use std::convert::TryInto;
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum Variant {
    Int(i32),
    Float(f64),
    Str(String),
}

pub type ParseResult<'a> = Result<ParsedFormat<'a, Variant>, usize>;

pub fn parse<'a, N>(format: &'a str, named: &'a N) -> ParseResult<'a>
where
    N: NamedArguments<Variant>,
{
    ParsedFormat::parse(format, &NoPositionalArguments, named)
}

impl FormatArgument for Variant {
    fn supports_format(&self, spec: &Specifier) -> bool {
        match self {
            Self::Int(_) => true,
            Self::Float(_) => match spec.format {
                Format::Display | Format::Debug | Format::LowerExp | Format::UpperExp => true,
                _ => false,
            },
            Self::Str(_) => match spec.format {
                Format::Display | Format::Debug => true,
                _ => false,
            },
        }
    }

    fn fmt_display(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Int(val) => fmt::Display::fmt(&val, f),
            Self::Float(val) => fmt::Display::fmt(&val, f),
            Self::Str(val) => fmt::Display::fmt(&val, f),
        }
    }

    fn fmt_debug(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }

    fn fmt_octal(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Int(val) => fmt::Octal::fmt(&val, f),
            _ => Err(fmt::Error),
        }
    }

    fn fmt_lower_hex(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Int(val) => fmt::LowerHex::fmt(&val, f),
            _ => Err(fmt::Error),
        }
    }

    fn fmt_upper_hex(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Int(val) => fmt::UpperHex::fmt(&val, f),
            _ => Err(fmt::Error),
        }
    }

    fn fmt_binary(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Int(val) => fmt::Binary::fmt(&val, f),
            _ => Err(fmt::Error),
        }
    }

    fn fmt_lower_exp(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Int(val) => fmt::LowerExp::fmt(&val, f),
            Self::Float(val) => fmt::LowerExp::fmt(&val, f),
            _ => Err(fmt::Error),
        }
    }

    fn fmt_upper_exp(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Int(val) => fmt::UpperExp::fmt(&val, f),
            Self::Float(val) => fmt::UpperExp::fmt(&val, f),
            _ => Err(fmt::Error),
        }
    }

    fn to_usize(&self) -> Result<usize, ()> {
        match self {
            Variant::Int(val) => (*val).try_into().map_err(|_| ()),
            Variant::Float(_) | Variant::Str(_) => Err(()),
        }
    }
}
