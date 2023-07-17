use std::borrow::Cow;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use eyre::{bail, WrapErr};

use crate::cli::AddNew;
use crate::cli::SourceType;
use crate::error::Error;
use crate::keyring::KeyLocation;

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
type OptionPair = (SourceOption, String);

/// A repository source.
#[derive(Debug)]
pub struct RepoSource {
    name: String,
    uri: String,
    description: Option<String>,
    suites: Vec<String>,
    components: Vec<String>,
    kind: Vec<SourceType>,
    key: Option<KeyLocation>,
    architectures: Vec<String>,
    languages: Vec<String>,
    enabled: bool,
    extra: Vec<OptionPair>,
}

/// Return the current distro version codename.
fn get_current_codename() -> eyre::Result<String> {
    Ok(String::from_utf8(
        Command::new("lsb_release")
            .arg("--short")
            .arg("--codename")
            .output()
            .wrap_err("failed getting distro version codename")?
            .stdout,
    )?)
}

/// Parse a custom option in `key=value` format.
fn parse_custom_option(option: String, force_literal: bool) -> eyre::Result<OptionPair> {
    let (key, value) = match option.trim().split_once('=') {
        Some(pair) => pair,
        None => bail!(Error::MalformedOption {
            option: option.to_string()
        }),
    };

    if force_literal {
        Ok((SourceOption::Custom(key.to_string()), value.to_string()))
    } else {
        Ok((
            SourceOption::Known(KnownSourceOption::from_str(key)?),
            value.to_string(),
        ))
    }
}

impl RepoSource {
    const KEYRING_DIR: &str = "/usr/share/keyrings";

    /// The path of a signing key for this source.
    pub fn key_path(&self) -> PathBuf {
        [
            Self::KEYRING_DIR,
            &format!("{}-archive-keyring.gpg", self.name),
        ]
        .iter()
        .collect()
    }

    /// Construct an instance from the CLI `args`.
    ///
    /// This does not download the signing key.
    pub fn from_cli(args: AddNew) -> eyre::Result<Self> {
        Ok(Self {
            name: args.name.clone(),
            uri: args.uri,
            description: args.description,
            suites: if args.suite.is_empty() {
                vec![get_current_codename()?]
            } else {
                args.suite
            },
            components: args.component,
            kind: args.kind,
            key: if let Some(url) = args.key.key_url {
                Some(KeyLocation::Download { url })
            } else if let Some(fingerprint) = args.key.fingerprint {
                Some(KeyLocation::Keyserver {
                    fingerprint,
                    keyserver: args.keyserver,
                })
            } else {
                None
            },
            architectures: args.arch,
            languages: args.lang,
            enabled: !args.disabled,
            extra: args
                .option
                .into_iter()
                .map(|option| parse_custom_option(option, args.force_literal_options))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
