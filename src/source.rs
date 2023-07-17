use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use clap::ValueEnum;
use eyre::{bail, WrapErr};

use crate::cli::AddNew;
use crate::error::Error;
use crate::keyring::KeyLocation;
use crate::option::KnownSourceOption;
use crate::option::{OptionPair, SourceOption};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SourceType {
    /// A binary package
    Deb,

    /// A source package
    DebSrc,
}

impl AsRef<str> for SourceType {
    fn as_ref(&self) -> &str {
        use SourceType::*;

        match self {
            Deb => "deb",
            DebSrc => "deb-src",
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
    pub uris: Vec<String>,
    pub suites: Vec<String>,
    pub components: Vec<String>,
    pub types: Vec<SourceType>,
    pub key: Option<KeyLocation>,
    pub enabled: bool,
    pub options: Vec<OptionPair>,
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
    const SOURCES_DIR: &str = "/etc/apt/sources.list.d";
    const KEYRING_DIR: &str = "/usr/share/keyrings";

    /// The path of this sources file.
    pub fn path(&self) -> PathBuf {
        [Self::SOURCES_DIR, &format!("{}.sources", self.name)]
            .iter()
            .collect()
    }

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
        let mut options = args
            .option
            .into_iter()
            .map(|option| parse_custom_option(option, args.force_literal_options))
            .collect::<Result<Vec<_>, _>>()?;

        if let Some(description) = args.description.description {
            options.push((
                SourceOption::Known(KnownSourceOption::RepolibName),
                description,
            ));
        }

        if !args.arch.is_empty() {
            options.push((
                SourceOption::Known(KnownSourceOption::Architectures),
                args.arch.join(" "),
            ));
        }

        if !args.lang.is_empty() {
            options.push((
                SourceOption::Known(KnownSourceOption::Languages),
                args.lang.join(" "),
            ));
        }

        Ok(Self {
            name: args.name.clone(),
            uris: args.uri,
            suites: if args.suite.is_empty() {
                vec![get_current_codename()?]
            } else {
                args.suite
            },
            components: args.component,
            types: args.kind,
            key: if let Some(url) = args.key.location.key_url {
                Some(KeyLocation::Download { url })
            } else if let Some(fingerprint) = args.key.location.fingerprint {
                Some(KeyLocation::Keyserver {
                    fingerprint,
                    keyserver: args.key.keyserver,
                })
            } else {
                None
            },
            enabled: !args.disabled.disabled,
            options,
        })
    }
}
