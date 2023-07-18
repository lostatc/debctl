use std::{borrow::Cow, collections::HashMap, str::FromStr};

use crate::error::Error;

/// The name of an option in a source file.
///
/// These are the known, valid option names listed in the sources.list(5) man page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnownOptionName {
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

impl KnownOptionName {
    /// The option name in deb822 syntax.
    pub fn to_deb822(self) -> &'static str {
        use KnownOptionName::*;

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

impl FromStr for KnownOptionName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use KnownOptionName::*;

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

/// The name of an option in a source file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OptionName {
    /// An option name listed in sources.list(5).
    Known(KnownOptionName),

    /// A custom option name provided by the user.
    Custom(String),
}

impl OptionName {
    /// The option name in deb822 syntax.
    pub fn to_deb822(&self) -> &str {
        use OptionName::*;

        match self {
            Known(option) => option.to_deb822(),
            Custom(option) => option,
        }
    }
}

impl From<KnownOptionName> for OptionName {
    fn from(value: KnownOptionName) -> Self {
        Self::Known(value)
    }
}

/// The value of an option in a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionValue {
    String(String),
    List(Vec<String>),
    Bool(bool),
}

impl From<String> for OptionValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<Vec<String>> for OptionValue {
    fn from(value: Vec<String>) -> Self {
        Self::List(value)
    }
}

impl From<bool> for OptionValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl OptionValue {
    /// The option value in deb822 syntax.
    pub fn to_deb822(&self) -> Cow<'_, str> {
        match self {
            Self::String(value) => Cow::Borrowed(value),
            Self::List(value) => Cow::Owned(value.join(" ")),
            Self::Bool(true) => Cow::Borrowed("yes"),
            Self::Bool(false) => Cow::Borrowed("no"),
        }
    }
}

/// A map of option names and their values.
#[derive(Debug)]
pub struct OptionMap(HashMap<OptionName, OptionValue>);

impl OptionMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Insert a new option into the map.
    ///
    /// If the option value is an empty string or an empty vector, then it is skipped.
    pub fn insert(&mut self, name: impl Into<OptionName>, value: impl Into<OptionValue>) {
        let option_name = name.into();
        let option_value = value.into();

        match option_value {
            OptionValue::List(list_value) if list_value.is_empty() => return,
            OptionValue::String(str_value) if str_value.is_empty() => return,
            _ => {}
        }

        self.0.insert(option_name, option_value);
    }

    /// Iterate over the key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&OptionName, &OptionValue)> {
        self.0.iter()
    }
}

impl FromIterator<(OptionName, OptionValue)> for OptionMap {
    fn from_iter<T: IntoIterator<Item = (OptionName, OptionValue)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}
