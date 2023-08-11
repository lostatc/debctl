use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;

use eyre::bail;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::error::Error;
use crate::key::SigningKey;
use crate::types::SourceType;

/// The name of an option in a source entry.
///
/// These are the known, valid option names listed in the sources.list(5) man page.
///
/// The order of the variants in this enum corresponds to the order the options will appear in
/// source entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, EnumIter)]
pub enum KnownOptionName {
    RepolibName,
    Enabled,
    Types,
    Uris,
    Suites,
    Components,
    SignedBy,
    Trusted,
    Architectures,
    Languages,
    Targets,
    PDiffs,
    ByHash,
    AllowInsecure,
    AllowWeak,
    AllowDowngradeToInsecure,
    CheckValidUntil,
    ValidUntilMin,
    ValidUntilMax,
}

impl KnownOptionName {
    /// The option name in deb822 syntax.
    pub const fn to_deb822(self) -> &'static str {
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

        // We accept option names as they appear in either the one-line syntax or the deb822 syntax,
        // without regard for case.
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

/// The name of an option in a source entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OptionName {
    /// An option name listed in sources.list(5).
    Known(KnownOptionName),

    /// A custom option name provided by the user.
    Custom(String),
}

impl OptionName {
    /// Return whether this is a known option.
    pub fn is_known(&self) -> bool {
        match self {
            Self::Known(_) => true,
            Self::Custom(_) => false,
        }
    }

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

impl From<&KnownOptionName> for OptionName {
    fn from(value: &KnownOptionName) -> Self {
        Self::Known(*value)
    }
}

/// Return whether this option value represents a true boolean value.
fn is_truthy(s: &str) -> bool {
    matches!(s.to_lowercase().as_str(), "yes" | "true")
}

/// Return whether this option value represents a false boolean value.
fn is_falsey(s: &str) -> bool {
    matches!(s.to_lowercase().as_str(), "no" | "false")
}

/// The value of an option in a source entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionValue {
    String(String),
    List(Vec<String>),
    Bool(bool),
    Multiline(Vec<String>),
}

impl From<Cow<'_, str>> for OptionValue {
    fn from(value: Cow<'_, str>) -> Self {
        if is_truthy(&value) {
            Self::Bool(true)
        } else if is_falsey(&value) {
            Self::Bool(false)
        } else {
            Self::String(value.into_owned())
        }
    }
}

impl From<String> for OptionValue {
    fn from(value: String) -> Self {
        Self::from(Cow::Owned(value))
    }
}

impl From<&String> for OptionValue {
    fn from(value: &String) -> Self {
        Self::from(Cow::Borrowed(value.as_str()))
    }
}

impl From<&str> for OptionValue {
    fn from(value: &str) -> Self {
        Self::from(Cow::Borrowed(value))
    }
}

impl From<Vec<&str>> for OptionValue {
    fn from(value: Vec<&str>) -> Self {
        value
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .into()
    }
}

impl From<Vec<String>> for OptionValue {
    fn from(value: Vec<String>) -> Self {
        match &value.as_slice() {
            &[single] => single.as_str().into(),
            _ => Self::List(value),
        }
    }
}

impl From<&Vec<String>> for OptionValue {
    fn from(value: &Vec<String>) -> Self {
        value.to_owned().into()
    }
}

impl From<Vec<SourceType>> for OptionValue {
    fn from(value: Vec<SourceType>) -> Self {
        value
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .into()
    }
}

impl From<&Vec<SourceType>> for OptionValue {
    fn from(value: &Vec<SourceType>) -> Self {
        value.to_owned().into()
    }
}

impl From<bool> for OptionValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl OptionValue {
    /// Return whether this value is "empty".
    ///
    /// Empty values are not included in the source entry.
    pub fn is_empty(&self) -> bool {
        match self {
            OptionValue::String(value) => value.trim().is_empty(),
            OptionValue::List(list) => list.is_empty(),
            OptionValue::Bool(_) => false,
            OptionValue::Multiline(list) => list.is_empty(),
        }
    }

    /// The option value in deb822 syntax.
    pub fn to_deb822(&self) -> Cow<'_, str> {
        match self {
            Self::String(value) => Cow::Borrowed(value),
            Self::List(value) => Cow::Owned(value.join(" ")),
            Self::Bool(true) => Cow::Borrowed("yes"),
            Self::Bool(false) => Cow::Borrowed("no"),
            Self::Multiline(lines) => {
                let mut output = String::new();

                // The first line should be blank.
                output.push('\n');

                for line in lines {
                    // Continuation lines for multiline strings start with a space.
                    output.push(' ');

                    if line.trim().is_empty() {
                        // Blank lines are escaped with a dot.
                        output.push('.');
                    } else {
                        output.push_str(line);
                    }

                    output.push('\n');
                }

                Cow::Owned(output)
            }
        }
    }
}

pub type OptionPair = (OptionName, OptionValue);

/// A map of option names and their values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionMap(HashMap<OptionName, OptionValue>);

impl OptionMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Insert the given option into the map.
    ///
    /// If the option value is empty, it is skipped.
    pub fn insert(&mut self, name: impl Into<OptionName>, value: impl Into<OptionValue>) {
        let option_name = name.into();
        let option_value = value.into();

        if !option_value.is_empty() {
            self.0.insert(option_name, option_value);
        }
    }

    /// Insert the signing key as an option.
    pub fn insert_key(&mut self, key: SigningKey) -> eyre::Result<()> {
        if self.contains(KnownOptionName::SignedBy) {
            bail!(Error::ConflictingKeyLocations);
        } else {
            let value: OptionValue = match key {
                SigningKey::File { path } => path.to_string_lossy().to_string().into(),
                SigningKey::Inline { value } => value,
            };

            self.insert(KnownOptionName::SignedBy, value);
        }

        Ok(())
    }

    /// Return whether this option map contains the given option.
    pub fn contains(&self, name: impl Into<OptionName>) -> bool {
        self.0.contains_key(&name.into())
    }

    /// Return the options in this map in their canonical order.
    ///
    /// Known options are ordered consistently. Custom options are sorted by their key and come
    /// after known options.
    pub fn options(&self) -> Vec<(&OptionName, &OptionValue)> {
        let mut custom_options = self
            .0
            .iter()
            .filter(|(name, _)| !name.is_known())
            .collect::<Vec<_>>();

        // We cannot use `Vec::sort_by_key` here because of the lifetimes.
        custom_options.sort_by(|(first, _), (second, _)| first.cmp(second));

        let mut all_options = Vec::with_capacity(self.0.len());

        for known_name in KnownOptionName::iter() {
            if let Some((key, value)) = self.0.get_key_value(&known_name.into()) {
                all_options.push((key, value));
            }
        }

        all_options.append(&mut custom_options);

        all_options
    }
}

impl FromIterator<(OptionName, OptionValue)> for OptionMap {
    fn from_iter<T: IntoIterator<Item = (OptionName, OptionValue)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use xpct::{be_err, be_ok, equal, expect};

    use crate::types::SourceType;

    use super::*;

    #[test]
    fn bool_option_values_are_formatted_correctly() {
        expect!(OptionValue::from(true))
            .map(|value| value.to_deb822().into_owned())
            .to(equal("yes"));

        expect!(OptionValue::from(false))
            .map(|value| value.to_deb822().into_owned())
            .to(equal("no"));
    }

    #[test]
    fn string_option_values_are_formatted_correctly() {
        expect!(OptionValue::from("foobar"))
            .map(|value| value.to_deb822().into_owned())
            .to(equal("foobar"));
    }

    #[test]
    fn list_option_values_are_formatted_correctly() {
        expect!(OptionValue::from(vec!["value1", "value2", "value3"]))
            .map(|value| value.to_deb822().into_owned())
            .to(equal("value1 value2 value3"));
    }

    #[test]
    fn multiline_option_values_are_formatted_correctly() {
        expect!(OptionValue::Multiline(vec![
            "value1".into(),
            "".into(),
            "value3".into()
        ]))
        .map(|value| value.to_deb822().into_owned())
        .to(equal("\n value1\n .\n value3\n"));
    }

    #[test]
    fn options_are_sorted_correctly() {
        let mut map = OptionMap::new();

        // Because the option map is internally backed by a hash map, and because the ordering of
        // hash maps is nondeterministic, this test could theoretically flake, succeeding when it
        // should fail. To reduce the statistical likelihood of this, we add a lot of options to the
        // option map.

        map.insert(OptionName::Custom("option-d".into()), "value-d");
        map.insert(OptionName::Custom("option-b".into()), "value-b");
        map.insert(OptionName::Custom("option-c".into()), "value-c");
        map.insert(OptionName::Custom("option-a".into()), "value-a");
        map.insert(KnownOptionName::Uris, vec!["https://example.com"]);
        map.insert(KnownOptionName::Types, vec![SourceType::Deb]);
        map.insert(KnownOptionName::Suites, vec!["jammy"]);
        map.insert(KnownOptionName::Enabled, true);

        expect!(map.options()).to(equal(vec![
            // Known options have a canonical sort order.
            (&KnownOptionName::Enabled.into(), &true.into()),
            (
                &KnownOptionName::Types.into(),
                &vec![SourceType::Deb].into(),
            ),
            (
                &KnownOptionName::Uris.into(),
                &vec!["https://example.com"].into(),
            ),
            (&KnownOptionName::Suites.into(), &vec!["jammy"].into()),
            // Custom options come after known options and are sorted by their option name.
            (&OptionName::Custom("option-a".into()), &"value-a".into()),
            (&OptionName::Custom("option-b".into()), &"value-b".into()),
            (&OptionName::Custom("option-c".into()), &"value-c".into()),
            (&OptionName::Custom("option-d".into()), &"value-d".into()),
        ]));
    }

    #[test]
    fn fails_on_conflicting_key_locations() {
        let mut map = OptionMap::new();

        map.insert(KnownOptionName::SignedBy, "/path/to/key.gpg");

        let result = map.insert_key(SigningKey::Inline {
            value: "encoded key bytes".into(),
        });

        expect!(result)
            .to(be_err())
            .map(|err| err.downcast::<Error>())
            .to(be_ok())
            .to(equal(Error::ConflictingKeyLocations));
    }
}
