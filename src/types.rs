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

impl ToString for SourceType {
    fn to_string(&self) -> String {
        match self {
            Self::Deb => String::from("deb"),
            Self::DebSrc => String::from("deb-src"),
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
