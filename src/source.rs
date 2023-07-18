use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use clap::ValueEnum;
use eyre::{bail, WrapErr};

use crate::cli::{AddLine, AddNew, SigningKeyArgs};
use crate::error::Error;
use crate::keyring::KeyLocation;
use crate::option::{KnownOptionName, OptionMap, OptionName, OptionValue};
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
fn parse_custom_option(
    option: String,
    force_literal: bool,
) -> eyre::Result<(OptionName, OptionValue)> {
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

    let lowercase_value = value.to_lowercase();

    let option_value = if lowercase_value == "yes" {
        OptionValue::Bool(true)
    } else if lowercase_value == "no" {
        OptionValue::Bool(false)
    } else if value.contains(',') {
        OptionValue::List(value.split(',').map(ToString::to_string).collect())
    } else {
        OptionValue::String(value.to_string())
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

const KEYRING_DIR: &str = "/usr/share/keyrings";

/// The path of a signing key for a repo source.
pub fn key_path(source_name: &str) -> PathBuf {
    [KEYRING_DIR, &format!("{}-archive-keyring.gpg", source_name)]
        .iter()
        .collect()
}

fn parse_key_args(args: SigningKeyArgs) -> Option<KeyLocation> {
    if let Some(url) = args.location.key_url {
        Some(KeyLocation::Download { url })
    } else if let Some(fingerprint) = args.location.fingerprint {
        Some(KeyLocation::Keyserver {
            fingerprint,
            keyserver: args.keyserver,
        })
    } else {
        None
    }
}

impl RepoSource {
    /// Construct an instance from the CLI `args`.
    ///
    /// This does not download the signing key.
    pub fn from_add_new_args(args: AddNew) -> eyre::Result<Self> {
        let mut options = args
            .option
            .into_iter()
            .map(|option| parse_custom_option(option, args.force_literal_options))
            .collect::<Result<OptionMap, _>>()?;

        let suites = if args.suite.is_empty() {
            vec![get_current_codename()?]
        } else {
            args.suite
        };

        let types = args
            .kind
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        options.insert(KnownOptionName::Uris, args.uri);

        options.insert(KnownOptionName::Suites, suites);

        options.insert(KnownOptionName::Types, types);

        options.insert(KnownOptionName::Components, args.component);

        options.insert(KnownOptionName::Architectures, args.arch);

        options.insert(KnownOptionName::Languages, args.lang);

        options.insert(KnownOptionName::Enabled, !args.disabled.disabled);

        if let Some(description) = args.description.description {
            options.insert(KnownOptionName::RepolibName, description);
        }

        let key = parse_key_args(args.key);

        options.insert_key(&args.name, &key);

        Ok(Self {
            name: args.name.clone(),
            options,
            key,
            overwrite: args.overwrite.overwrite,
        })
    }

    pub fn from_add_line_args(args: AddLine) -> eyre::Result<Self> {
        let mut options = parse_line_entry(&args.line)?;

        options.insert(KnownOptionName::Enabled, !args.disabled.disabled);

        if let Some(description) = args.description.description {
            options.insert(KnownOptionName::RepolibName, description);
        }

        let key = parse_key_args(args.key);

        options.insert_key(&args.name, &key);

        Ok(Self {
            name: args.name.clone(),
            options,
            key,
            overwrite: args.overwrite.overwrite,
        })
    }
}
