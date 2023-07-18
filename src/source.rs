use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use clap::ValueEnum;
use eyre::{bail, WrapErr};

use crate::cli::AddNew;
use crate::error::Error;
use crate::keyring::KeyLocation;
use crate::option::{KnownOptionName, OptionMap, OptionName, OptionValue};

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

    Ok((option_name, OptionValue::String(value.to_string())))
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

impl RepoSource {
    /// Construct an instance from the CLI `args`.
    ///
    /// This does not download the signing key.
    pub fn from_cli(args: AddNew) -> eyre::Result<Self> {
        let mut options = args
            .option
            .into_iter()
            .map(|option| parse_custom_option(option, args.force_literal_options))
            .collect::<Result<OptionMap, _>>()?;

        options.insert(KnownOptionName::Uris, args.uri);

        options.insert(KnownOptionName::Components, args.component);

        options.insert(KnownOptionName::Architectures, args.arch);

        options.insert(KnownOptionName::Languages, args.lang);

        options.insert(KnownOptionName::Enabled, !args.disabled.disabled);

        options.insert(
            KnownOptionName::Suites,
            if args.suite.is_empty() {
                vec![get_current_codename()?]
            } else {
                args.suite
            },
        );

        options.insert(
            KnownOptionName::Types,
            args.kind
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        );

        if let Some(description) = args.description.description {
            options.insert(KnownOptionName::RepolibName, description);
        }

        let key = if let Some(url) = args.key.location.key_url {
            Some(KeyLocation::Download { url })
        } else if let Some(fingerprint) = args.key.location.fingerprint {
            Some(KeyLocation::Keyserver {
                fingerprint,
                keyserver: args.key.keyserver,
            })
        } else {
            None
        };

        if key.is_some() {
            options.insert(
                KnownOptionName::SignedBy,
                key_path(&args.name).to_string_lossy().to_string(),
            );
        }

        Ok(Self {
            name: args.name.clone(),
            options,
            key,
            overwrite: args.overwrite.overwrite,
        })
    }
}
