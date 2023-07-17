use std::borrow::Cow;
use std::str::FromStr;

use crate::error::Error;

/// A valid, known option in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownSourceOption {
    Types,
    Uris,
    Suites,
    Components,
    Enabled,
    Architectures,
    Languages,
    Targets,
    PDiffs,
    ByHash,
    AllowInsecure,
    AllowWeak,
    AllowDowngradeToInsecure,
    Trusted,
    SignedBy,
    CheckValidUntil,
    ValidUntilMin,
    ValidUntilMax,
    RepolibName,
}

impl KnownSourceOption {
    /// The option name in deb822 syntax.
    pub fn to_deb822(self) -> &'static str {
        use KnownSourceOption::*;

        match self {
            Types => "Types",
            Uris => "URIs",
            Suites => "Suites",
            Components => "Components",
            Enabled => "Enabled",
            Architectures => "Architectures",
            Languages => "Languages",
            Targets => "Targets",
            PDiffs => "PDiffs",
            ByHash => "By-Hash",
            AllowInsecure => "Allow-Insecure",
            AllowWeak => "Allow-Weak",
            AllowDowngradeToInsecure => "Allow-Downgrade-To-Insecure",
            Trusted => "Trusted",
            SignedBy => "Signed-By",
            CheckValidUntil => "Check-Valid-Until",
            ValidUntilMin => "Valid-Until-Min",
            ValidUntilMax => "Valid-Until-Max",
            RepolibName => "X-Repolib-Name",
        }
    }
}

impl FromStr for KnownSourceOption {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use KnownSourceOption::*;

        // We accept option names as they appear in either the single-line syntax or the deb822
        // syntax, without regard for case.
        match s.to_lowercase().as_str() {
            "types" => Ok(Types),
            "uris" => Ok(Uris),
            "suites" => Ok(Suites),
            "components" => Ok(Components),
            "enabled" => Ok(Enabled),
            "architectures" | "arch" => Ok(Architectures),
            "languages" | "lang" => Ok(Languages),
            "targets" | "target" => Ok(Targets),
            "pdiffs" => Ok(PDiffs),
            "by-hash" => Ok(ByHash),
            "allow-insecure" => Ok(AllowInsecure),
            "allow-weak" => Ok(AllowWeak),
            "allow-downgrade-to-insecure" => Ok(AllowDowngradeToInsecure),
            "trusted" => Ok(Trusted),
            "signed-by" => Ok(SignedBy),
            "check-valid-until" => Ok(CheckValidUntil),
            "valid-until-min" => Ok(ValidUntilMin),
            "valid-until-max" => Ok(ValidUntilMax),
            _ => Err(Error::InvalidOptionName {
                name: s.to_string(),
            }),
        }
    }
}

/// An option in a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOption {
    /// An option which is known to be valid.
    Known(KnownSourceOption),

    /// A custom option provided by the user.
    Custom(String),
}

impl SourceOption {
    /// The option name in deb822 syntax.
    pub fn into_deb822(self) -> Cow<'static, str> {
        use SourceOption::*;

        match self {
            Known(option) => Cow::Borrowed(option.to_deb822()),
            Custom(option) => Cow::Owned(option),
        }
    }
}

/// An option in a source file and its value.
pub type OptionPair = (SourceOption, String);
