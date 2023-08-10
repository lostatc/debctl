//! Validated CLI args.
//!
//! The types in the module map to the CLI args passed by the user, but modeled in such a way as to
//! enforce required and mutually exclusive options.
//!
//! This module also does additional input validation beyond what's possible with `clap`, and
//! double-checks some of the input validation done by `clap` as a safeguard.

use std::path::{Path, PathBuf};

use eyre::bail;
use reqwest::Url;

use crate::cli;
use crate::codename::get_version_codename;
use crate::error::Error;
use crate::option::OptionMap;
use crate::parse::parse_custom_option;
use crate::types::SourceType;

#[derive(Debug, Clone)]
pub enum KeySource {
    NoKey,
    Url { url: Url },
    File { path: PathBuf },
    Keyserver { id: String, keyserver: Url },
}

impl KeySource {
    fn from_cli(args: &cli::SigningKeyArgs) -> eyre::Result<Self> {
        Ok(match (&args.location.key, args.location.force_no_key) {
            (None, true) => Self::NoKey,
            (None, false) => bail!("must either specify a key or force no key"),
            (Some(_), true) => bail!("cannot both specify a key and force no key"),
            (Some(key), false) => {
                if let Some(keyserver) = &args.keyserver {
                    Self::Keyserver {
                        id: key.to_owned(),
                        keyserver: Url::parse(keyserver.as_str())?,
                    }
                } else if let Ok(url) = Url::parse(key.as_str()) {
                    Self::Url { url }
                } else {
                    let key_path = Path::new(key.as_str());

                    if key_path.exists() {
                        Self::File {
                            path: key_path.to_path_buf(),
                        }
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

#[derive(Debug, Clone)]
pub enum KeyDest {
    Inline,
    File { path: PathBuf },
}

impl KeyDest {
    const DEFAULT_KEYRING_DIR: &str = "/etc/apt/keyrings";

    fn from_cli(args: &cli::KeyDestinationArgs, name: &str) -> eyre::Result<Self> {
        Ok(match (&args.key_path, args.inline_key) {
            (None, true) => Self::Inline,
            (None, false) => Self::File {
                path: [
                    Self::DEFAULT_KEYRING_DIR,
                    &format!("{}-archive-keyring.gpg", name),
                ]
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
    fn from_cli(args: cli::OverwriteArgs) -> eyre::Result<Self> {
        Ok(match (args.overwrite, args.append) {
            (true, true) => bail!("cannot both overwrite and append"),
            (true, false) => Self::Overwrite,
            (false, true) => Self::Append,
            (false, false) => Self::Fail,
        })
    }
}

#[derive(Debug, Clone)]
pub struct KeyArgs {
    pub source: KeySource,
    pub dest: KeyDest,
}

impl KeyArgs {
    fn from_cli(args: &cli::SigningKeyArgs, name: &str) -> eyre::Result<Self> {
        Ok(Self {
            source: KeySource::from_cli(args)?,
            dest: KeyDest::from_cli(&args.destination, name)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct NewArgs {
    pub name: String,
    pub uris: Vec<Url>,
    pub description: Option<String>,
    pub suites: Vec<String>,
    pub components: Vec<String>,
    pub kinds: Vec<SourceType>,
    pub key: KeyArgs,
    pub arch: Vec<String>,
    pub lang: Vec<String>,
    pub options: OptionMap,
    pub disabled: bool,
    pub action: OverwriteAction,
}

impl NewArgs {
    /// Parse and validate the CLI args.
    ///
    /// As a precaution, this validates some things that *should* already be validated by `clap`.
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
}

#[derive(Debug, Clone)]
pub struct AddArgs {
    pub line: String,
    pub name: String,
    pub description: Option<String>,
    pub key: KeyArgs,
    pub disabled: bool,
    pub action: OverwriteAction,
}

impl AddArgs {
    pub fn from_cli(args: cli::Add) -> eyre::Result<Self> {
        Ok(Self {
            line: args.line,
            name: args.name.clone(),
            description: args.description.description,
            key: KeyArgs::from_cli(&args.key, &args.name)?,
            disabled: args.disabled.disabled,
            action: OverwriteAction::from_cli(args.overwrite)?,
        })
    }
}

#[derive(Debug, Clone)]
pub enum BackupStrategy {
    Backup,
    BackupTo { path: PathBuf },
}

impl BackupStrategy {
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

#[derive(Debug, Clone)]
pub enum ConvertLocator {
    Name {
        name: String,
        backup: Option<BackupStrategy>,
    },
    File {
        source: PathBuf,
        dest: PathBuf,
    },
}

impl ConvertLocator {
    fn from_cli(args: &cli::Convert) -> eyre::Result<Self> {
        Ok(if let Some(name) = &args.name {
            if args.in_path.is_some() || args.out_path.is_some() {
                bail!("cannot specify both a source name and in/out file paths")
            }

            Self::Name {
                name: name.to_owned(),
                backup: BackupStrategy::from_cli(&args)?,
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

#[derive(Debug, Clone)]
pub struct ConvertArgs {
    pub locator: ConvertLocator,
    pub skip_comments: bool,
    pub skip_disabled: bool,
}

impl ConvertArgs {
    pub fn from_cli(args: &cli::Convert) -> eyre::Result<Self> {
        Ok(Self {
            locator: ConvertLocator::from_cli(args)?,
            skip_comments: args.skip_comments,
            skip_disabled: args.skip_disabled,
        })
    }
}
