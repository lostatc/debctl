//! Validated CLI args.
//!
//! The types in the module map to the CLI args passed by the user, but modeled in such a way as to
//! enforce required and mutually exclusive options.
//!
//! This module also does additional input validation beyond what's possible with `clap`, and
//! double-checks some of the input validation done by `clap` as a safeguard.

use std::path::{Path, PathBuf};

use eyre::{bail, WrapErr};
use reqwest::Url;

use crate::cli;
use crate::codename::get_version_codename;
use crate::error::Error;
use crate::key::{KeyDest, KeySource};
use crate::option::{KnownOptionName, OptionMap};
use crate::parse::{parse_custom_option, parse_line_entry};
use crate::types::SourceType;

impl KeySource {
    /// Parse and validate CLI args.
    fn from_cli(args: &cli::SigningKeyArgs) -> eyre::Result<Option<Self>> {
        Ok(match (&args.location.key, args.location.force_no_key) {
            (None, true) => None,
            (None, false) => bail!("must either specify a key or force no key"),
            (Some(_), true) => bail!("cannot both specify a key and force no key"),
            (Some(key), false) => {
                if let Some(keyserver) = &args.keyserver {
                    Some(Self::Keyserver {
                        id: key.to_owned(),
                        keyserver: keyserver.to_owned(),
                    })
                } else if let Ok(url) = Url::parse(key.as_str()) {
                    Some(Self::Download { url })
                } else {
                    let key_path = Path::new(key.as_str());

                    if key_path.exists() {
                        Some(Self::File {
                            path: key_path.to_path_buf(),
                        })
                    } else {
                        bail!(Error::InvalidKeyLocation {
                            path: key.to_string()
                        });
                    }
                }
            }
        })
    }
}

impl KeyDest {
    const DEFAULT_KEYRING_DIR: &str = "/etc/apt/keyrings";

    /// Parse and validate CLI args.
    fn from_cli(args: &cli::KeyDestinationArgs, name: &str) -> eyre::Result<Self> {
        Ok(match (&args.key_path, args.inline_key) {
            (None, true) => Self::Inline,
            (None, false) => Self::File {
                path: [Self::DEFAULT_KEYRING_DIR, &format!("{}.gpg", name)]
                    .iter()
                    .collect(),
            },
            (Some(_), true) => bail!("cannot both inline key and install it to a file"),
            (Some(path), false) => Self::File {
                path: path.to_owned(),
            },
        })
    }
}

/// What to do if a repo source file already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverwriteAction {
    Overwrite,
    Append,
    Fail,
}

impl OverwriteAction {
    /// Parse and validate CLI args.
    fn from_cli(args: cli::OverwriteArgs) -> eyre::Result<Self> {
        Ok(match (args.overwrite, args.append) {
            (true, true) => bail!("cannot both overwrite and append"),
            (true, false) => Self::Overwrite,
            (false, true) => Self::Append,
            (false, false) => Self::Fail,
        })
    }
}

/// Args for locating a key and where to install it.
#[derive(Debug, Clone)]
pub struct KeyArgs {
    pub source: Option<KeySource>,
    pub dest: KeyDest,
}

impl KeyArgs {
    /// Parse and validate CLI args.
    fn from_cli(args: &cli::SigningKeyArgs, name: &str) -> eyre::Result<Self> {
        Ok(Self {
            source: KeySource::from_cli(args)?,
            dest: KeyDest::from_cli(&args.destination, name)?,
        })
    }
}

/// Args for creating a new repo source entry.
#[derive(Debug, Clone)]
pub struct NewArgs {
    name: String,
    kinds: Vec<SourceType>,
    uris: Vec<Url>,
    description: Option<String>,
    suites: Vec<String>,
    components: Vec<String>,
    arch: Vec<String>,
    lang: Vec<String>,
    disabled: bool,
    options: OptionMap,
    key: KeyArgs,
    action: OverwriteAction,
}

impl NewArgs {
    /// Parse and validate CLI args.
    pub fn from_cli(args: cli::New) -> eyre::Result<Self> {
        Ok(Self {
            name: args.name.clone(),
            uris: args
                .uri
                .into_iter()
                .map(|uri| {
                    Url::parse(uri.as_str()).map_err(|err| Error::MalformedUri {
                        uri,
                        reason: err.to_string(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            description: args.description.description,
            suites: if args.suite.is_empty() {
                vec![get_version_codename()?]
            } else {
                args.suite
            },
            components: args.component,
            kinds: if args.kind.is_empty() {
                bail!("must specify at least one source kind")
            } else {
                args.kind
            },
            key: KeyArgs::from_cli(&args.key, &args.name)?,
            arch: args.arch,
            lang: args.lang,
            options: args
                .option
                .iter()
                .map(|option| parse_custom_option(option, args.force_literal_options))
                .collect::<Result<OptionMap, _>>()?,
            disabled: args.disabled.disabled,
            action: OverwriteAction::from_cli(args.overwrite)?,
        })
    }

    /// The full map of source entry options passed at the CLI.
    pub fn options(&self) -> OptionMap {
        let mut options = self.options.clone();

        options.insert(KnownOptionName::Types, &self.kinds);

        options.insert(
            KnownOptionName::Uris,
            &self
                .uris
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        );

        if let Some(description) = &self.description {
            options.insert(KnownOptionName::RepolibName, description);
        }

        options.insert(KnownOptionName::Suites, &self.suites);

        options.insert(KnownOptionName::Components, &self.components);

        options.insert(KnownOptionName::Architectures, &self.arch);

        options.insert(KnownOptionName::Languages, &self.lang);

        options.insert(KnownOptionName::Enabled, !self.disabled);

        options
    }

    /// The parameters for the signing key passed at the CLI.
    pub fn key(&self) -> &KeyArgs {
        &self.key
    }

    /// The name of the source entry.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// The action to take if the source file already exists.
    pub fn action(&self) -> OverwriteAction {
        self.action
    }
}

/// Args for creating a new repo source entry from a single-line entry.
#[derive(Debug, Clone)]
pub struct AddArgs {
    name: String,
    line: String,
    description: Option<String>,
    key: KeyArgs,
    disabled: bool,
    action: OverwriteAction,
}

impl AddArgs {
    /// Parse and validate CLI args.
    pub fn from_cli(args: cli::Add) -> eyre::Result<Self> {
        Ok(Self {
            name: args.name.clone(),
            line: args.line,
            description: args.description.description,
            key: KeyArgs::from_cli(&args.key, &args.name)?,
            disabled: args.disabled.disabled,
            action: OverwriteAction::from_cli(args.overwrite)?,
        })
    }

    /// The full map of source entry options passed at the CLI.
    pub fn options(&self) -> eyre::Result<OptionMap> {
        let mut options =
            parse_line_entry(&self.line).wrap_err("failed parsing one-line-style source entry")?;

        options.insert(KnownOptionName::Enabled, !self.disabled);

        if let Some(description) = &self.description {
            options.insert(KnownOptionName::RepolibName, description);
        }

        Ok(options)
    }

    /// The parameters for the signing key passed at the CLI.
    pub fn key(&self) -> &KeyArgs {
        &self.key
    }

    /// The name of the source entry.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// The action to take if the source file already exists.
    pub fn action(&self) -> OverwriteAction {
        self.action
    }
}

/// How to back up a source file when converting.
#[derive(Debug, Clone)]
pub enum BackupMode {
    Backup,
    BackupTo { path: PathBuf },
}

impl BackupMode {
    fn from_cli(args: &cli::Convert) -> eyre::Result<Option<Self>> {
        Ok(match (args.backup, &args.backup_to) {
            (true, None) => Some(Self::Backup),
            (true, Some(_)) => {
                bail!("cannot both backup to the default path and also backup to a different path")
            }
            (false, None) => None,
            (false, Some(path)) => Some(Self::BackupTo {
                path: path.to_owned(),
            }),
        })
    }
}

/// The locations of the source and destination files when converting.
#[derive(Debug, Clone)]
pub enum ConvertLocator {
    Name {
        name: String,
        backup: Option<BackupMode>,
    },
    File {
        source: PathBuf,
        dest: PathBuf,
    },
}

impl ConvertLocator {
    /// Parse and validate CLI args.
    fn from_cli(args: &cli::Convert) -> eyre::Result<Self> {
        Ok(if let Some(name) = &args.name {
            if args.in_path.is_some() || args.out_path.is_some() {
                bail!("cannot specify both a source name and in/out file paths")
            }

            Self::Name {
                name: name.to_owned(),
                backup: BackupMode::from_cli(args)?,
            }
        } else {
            match (&args.in_path, &args.out_path) {
                (Some(source), Some(dest)) => Self::File {
                    source: source.to_owned(),
                    dest: dest.to_owned(),
                },
                _ => bail!("must specify either a source name or both of the in/out file paths"),
            }
        })
    }
}

/// Args for converting repo source files.
#[derive(Debug, Clone)]
pub struct ConvertArgs {
    locator: ConvertLocator,
    skip_comments: bool,
    skip_disabled: bool,
}

impl ConvertArgs {
    /// Parse and validate CLI args.
    pub fn from_cli(args: &cli::Convert) -> eyre::Result<Self> {
        Ok(Self {
            locator: ConvertLocator::from_cli(args)?,
            skip_comments: args.skip_comments,
            skip_disabled: args.skip_disabled,
        })
    }

    /// The locations of the source and destination files when converting.
    pub fn locator(&self) -> &ConvertLocator {
        &self.locator
    }

    /// How to back up the original file.
    pub fn backup_mode(&self) -> Option<&BackupMode> {
        match &self.locator {
            ConvertLocator::Name { backup, .. } => backup.as_ref(),
            ConvertLocator::File { .. } => None,
        }
    }

    /// Skip transferring comment lines when converting.
    pub fn skip_comments(&self) -> bool {
        self.skip_comments
    }

    /// Skip converting commented-out entries to disabled entries.
    pub fn skip_disabled(&self) -> bool {
        self.skip_disabled
    }
}
