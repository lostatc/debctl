use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use clap::ValueEnum;
use eyre::{bail, WrapErr};
use reqwest::Url;

use crate::cli::{AddLine, AddNew, SigningKeyArgs};
use crate::error::Error;
use crate::keyring::KeyLocation;
use crate::option::{KnownOptionName, OptionMap, OptionName, OptionPair, OptionValue};
use crate::parse::parse_line_entry;

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
            _ => Err(Error::MalformedSingleLineEntry {
                reason: String::from("The entry must start with `deb` or `deb-src`."),
            }),
        }
    }
}

/// A repository source.
#[derive(Debug)]
pub struct RepoSource {
    pub name: String,
    pub options: OptionMap,
    pub key: Option<KeyLocation>,
    pub keyring_dir: PathBuf,
    pub overwrite: bool,
}

/// Return the current distro version codename.
fn get_current_codename() -> eyre::Result<String> {
    let stdout = Command::new("lsb_release")
        .arg("--short")
        .arg("--codename")
        .output()
        .wrap_err("failed getting distro version codename")?
        .stdout;

    Ok(String::from_utf8(stdout)?.trim().to_string())
}

/// Parse a custom option in `key=value` format.
fn parse_custom_option(option: String, force_literal: bool) -> eyre::Result<OptionPair> {
    let (key, value) = match option.trim().split_once('=') {
        Some(pair) => pair,
        None => bail!(Error::MalformedOption {
            option: option.to_string()
        }),
    };

    let option_name = if force_literal {
        OptionName::Custom(key.to_string())
    } else {
        KnownOptionName::from_str(key)?.into()
    };

    let option_value: OptionValue = if value.contains(',') {
        value
            .split(',')
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .into()
    } else {
        value.to_string().into()
    };

    Ok((option_name, option_value))
}

const SOURCES_DIR: &str = "/etc/apt/sources.list.d";

/// The path of a repo source file.
pub fn source_path(source_name: &str) -> PathBuf {
    [SOURCES_DIR, &format!("{}.sources", source_name)]
        .iter()
        .collect()
}

/// The path of a signing key for a repo source.
pub fn key_path(keyring_dir: &Path, source_name: &str) -> PathBuf {
    keyring_dir.join(format!("{}-archive-keyring.gpg", source_name))
}

/// Parse the CLI args to determine where we're fetching the singing key from.
fn parse_key_args(args: SigningKeyArgs) -> eyre::Result<Option<KeyLocation>> {
    Ok(match (args.location.key, args.keyserver) {
        (Some(key_location), Some(keyserver)) => Some(KeyLocation::Keyserver {
            fingerprint: key_location,
            keyserver,
        }),
        (Some(key_location), None) => match Url::parse(&key_location) {
            Ok(url) => Some(KeyLocation::Download { url }),
            Err(_) => {
                let path = Path::new(&key_location);

                if path.exists() {
                    Some(KeyLocation::File {
                        path: path.to_path_buf(),
                    })
                } else {
                    bail!(Error::InvalidKeyLocation {
                        path: key_location.to_string()
                    });
                }
            }
        },
        (None, _) => None,
    })
}

impl RepoSource {
    /// The path of a signing key for this repo source.
    pub fn key_path(&self) -> PathBuf {
        key_path(&self.keyring_dir, &self.name)
    }

    /// Construct an instance from the CLI `args`.
    ///
    /// This does not download the signing key.
    pub fn from_add_new_args(args: AddNew) -> eyre::Result<Self> {
        let mut options = args
            .option
            .into_iter()
            .map(|option| parse_custom_option(option, args.force_literal_options))
            .collect::<Result<OptionMap, _>>()?;

        options.insert(KnownOptionName::Uris, args.uri);

        options.insert(KnownOptionName::Types, args.kind);

        options.insert(KnownOptionName::Components, args.component);

        options.insert(KnownOptionName::Architectures, args.arch);

        options.insert(KnownOptionName::Languages, args.lang);

        options.insert(KnownOptionName::Enabled, !args.disabled.disabled);

        options.insert_or_else(KnownOptionName::Suites, args.suite, get_current_codename)?;

        options.insert_if_some(KnownOptionName::RepolibName, args.description.description);

        let keyring_dir = Path::new(&args.key.keyring_dir).to_path_buf();
        let key = parse_key_args(args.key)?;

        if key.is_some() {
            options.insert_key(&keyring_dir, &args.name)?;
        }

        Ok(Self {
            name: args.name.clone(),
            options,
            key,
            keyring_dir,
            overwrite: args.overwrite.overwrite,
        })
    }

    pub fn from_add_line_args(args: AddLine) -> eyre::Result<Self> {
        let mut options = parse_line_entry(&args.line)
            .wrap_err("failed parsing single-line-style source entry")?;

        options.insert(KnownOptionName::Enabled, !args.disabled.disabled);

        options.insert_if_some(KnownOptionName::RepolibName, args.description.description);

        let keyring_dir = Path::new(&args.key.keyring_dir).to_path_buf();
        let key = parse_key_args(args.key)?;

        if key.is_some() {
            options.insert_key(&keyring_dir, &args.name)?;
        }

        Ok(Self {
            name: args.name.clone(),
            options,
            key,
            keyring_dir,
            overwrite: args.overwrite.overwrite,
        })
    }
}
