use std::fmt::{self, Display};
use std::str::FromStr;

use clap::ValueEnum;

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SourceType {
    /// A binary package
    Deb,

    /// A source package
    DebSrc,
}

impl Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Deb => f.write_str("deb"),
            Self::DebSrc => f.write_str("deb-src"),
        }
    }
}

impl FromStr for SourceType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use SourceType::*;

        match s {
            "deb" => Ok(Deb),
            "deb-src" => Ok(DebSrc),
            _ => Err(Error::MalformedOneLineEntry {
                reason: String::from("The entry must start with `deb` or `deb-src`."),
            }),
        }
    }
}
