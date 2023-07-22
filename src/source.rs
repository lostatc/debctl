use std::borrow::Cow;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use eyre::{bail, WrapErr};
use reqwest::Url;

use crate::cli::{Add, KeyDestinationArgs, New, OverwriteArgs, SigningKeyArgs};
use crate::error::Error;
use crate::key::KeySource;
use crate::option::{KnownOptionName, OptionMap, OptionValue};
use crate::parse::{parse_custom_option, parse_line_entry};

/// The location to install a signing key to.
#[derive(Debug)]
pub enum KeyDestination {
    File { path: PathBuf },
    Inline,
}

/// The path of the signing key for a source entry.
fn key_path(keyring_dir: &Path, name: String) -> PathBuf {
    keyring_dir.join(format!("{}-archive-keyring.gpg", name))
}

impl KeyDestination {
    /// Construct an instance from CLI args.
    pub fn from_args(args: &KeyDestinationArgs, name: &str) -> KeyDestination {
        if args.inline_key {
            KeyDestination::Inline
        } else {
            KeyDestination::File {
                path: key_path(&args.keyring_dir, name.to_owned()),
            }
        }
    }
}

/// A repository signing key.
#[derive(Debug)]
pub enum SigningKey {
    /// The key is stored in a separate file.
    File { path: PathBuf },

    /// The key is inlined in the source file.
    Inline { value: OptionValue },
}

/// What to do if a source file already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverwriteAction {
    Overwrite,
    Append,
    Fail,
}

impl OverwriteArgs {
    /// The `OverwriteAction` for these args.
    pub fn action(&self) -> OverwriteAction {
        if self.overwrite {
            OverwriteAction::Overwrite
        } else if self.append {
            OverwriteAction::Append
        } else {
            OverwriteAction::Fail
        }
    }
}

/// The path of a repo source file.
#[derive(Debug)]
pub enum SourceFilePath {
    /// A source file installed in the APT sources directory.
    Installed { name: String },

    /// A source file at an arbitrary file path.
    File { path: PathBuf },
}

/// A kind of source file.
#[derive(Debug, Clone, Copy)]
pub enum SourceFileKind {
    /// A single-line-style source file.
    SingleLine,

    /// A deb822-style source file.
    Deb822,
}

/// A repo source file.
#[derive(Debug)]
pub struct SourceFile {
    pub path: SourceFilePath,
    pub kind: SourceFileKind,
}

impl SourceFile {
    const SOURCES_DIR: &str = "/etc/apt/sources.list.d";

    /// The path of this source file.
    pub fn path(&self) -> Cow<'_, Path> {
        let extension = match self.kind {
            SourceFileKind::SingleLine => "list",
            SourceFileKind::Deb822 => "sources",
        };

        match &self.path {
            SourceFilePath::Installed { name } => Cow::Owned(
                [Self::SOURCES_DIR, &format!("{}.{}", name, extension)]
                    .iter()
                    .collect(),
            ),
            SourceFilePath::File { path } => Cow::Borrowed(path),
        }
    }
}

/// A repository source entry.
#[derive(Debug)]
pub struct SourceEntry {
    file: SourceFile,
    options: OptionMap,
    key: Option<KeySource>,
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

impl SigningKeyArgs {
    /// Where we're fetching the signing key from.
    pub fn key_source(&self) -> eyre::Result<Option<KeySource>> {
        Ok(match (&self.location.key, &self.keyserver) {
            (Some(key_location), Some(keyserver)) => Some(KeySource::Keyserver {
                id: key_location.to_string(),
                keyserver: keyserver.to_string(),
            }),
            (Some(key_location), None) => match Url::parse(key_location) {
                Ok(url) => Some(KeySource::Download { url }),
                Err(_) => {
                    let path = Path::new(&key_location);

                    if path.exists() {
                        Some(KeySource::File {
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
}

impl SourceEntry {
    /// Construct an instance from the CLI `args`.
    pub fn from_new_args(args: New) -> eyre::Result<Self> {
        let mut options = args
            .option
            .into_iter()
            .map(|option| parse_custom_option(&option, args.force_literal_options))
            .collect::<Result<OptionMap, _>>()?;

        options.insert(KnownOptionName::Uris, args.uri);

        options.insert(KnownOptionName::Types, args.kind);

        options.insert(KnownOptionName::Components, args.component);

        options.insert(KnownOptionName::Architectures, args.arch);

        options.insert(KnownOptionName::Languages, args.lang);

        options.insert(KnownOptionName::Enabled, !args.disabled.disabled);

        options.insert_or_else(KnownOptionName::Suites, args.suite, get_current_codename)?;

        if let Some(description) = args.description.description {
            options.insert(KnownOptionName::RepolibName, description);
        }

        Ok(Self {
            file: SourceFile {
                path: SourceFilePath::Installed {
                    name: args.name.clone(),
                },
                kind: SourceFileKind::Deb822,
            },
            options,
            key: args.key.key_source()?,
        })
    }

    /// Construct an instance from the CLI `args`.
    pub fn from_add_args(args: Add) -> eyre::Result<Self> {
        let mut options = parse_line_entry(&args.line)
            .wrap_err("failed parsing single-line-style source entry")?;

        options.insert(KnownOptionName::Enabled, !args.disabled.disabled);

        if let Some(description) = args.description.description {
            options.insert(KnownOptionName::RepolibName, description);
        }

        Ok(Self {
            file: SourceFile {
                path: SourceFilePath::Installed {
                    name: args.name.clone(),
                },
                kind: SourceFileKind::Deb822,
            },
            options,
            key: args.key.key_source()?,
        })
    }

    /// Install the key for this source entry.
    pub fn install_key(&mut self, dest: KeyDestination) -> eyre::Result<()> {
        if let Some(key_location) = &self.key {
            let key = match dest {
                KeyDestination::File { path } => {
                    key_location
                        .install(&path)
                        .wrap_err("failed installing signing key to file")?;

                    SigningKey::File { path }
                }
                KeyDestination::Inline => SigningKey::Inline {
                    value: key_location
                        .to_value()
                        .wrap_err("failed installing inline signing key")?,
                },
            };

            self.options.insert_key(key)?;
        }

        Ok(())
    }

    /// Open the repo source file.
    fn open_source_file(&self, path: &Path, action: OverwriteAction) -> eyre::Result<File> {
        let result = match action {
            OverwriteAction::Overwrite => OpenOptions::new()
                .create(true)
                .truncate(true)
                .read(true)
                .write(true)
                .open(path),
            OverwriteAction::Append => OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(path),
            OverwriteAction::Fail => OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(path),
        };

        match result {
            Ok(file) => Ok(file),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                bail!(Error::NewSourceFileAlreadyExists {
                    path: path.to_owned()
                })
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                bail!(Error::PermissionDenied)
            }
            Err(err) => bail!(err),
        }
    }

    /// Install this source entry as a file in deb822 format.
    pub fn install(&self, action: OverwriteAction) -> eyre::Result<()> {
        let mut file = self.open_source_file(&self.file.path(), action)?;

        if action == OverwriteAction::Append {
            let last_line = BufReader::new(&mut file).lines().last().transpose()?;

            file.seek(SeekFrom::End(0))?;

            // Stanzas in a deb822 file must have a blank line between them, but we don't want to
            // add unnecessary blank lines if the file already ends with one.
            if let Some(line) = last_line {
                if !line.trim().is_empty() {
                    writeln!(&mut file)?;
                }
            }
        }

        for (key, value) in self.options.options() {
            writeln!(&mut file, "{}: {}", key.to_deb822(), value.to_deb822())?;
        }

        Ok(())
    }
}
