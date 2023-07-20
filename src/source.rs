use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use eyre::{bail, WrapErr};
use reqwest::Url;

use crate::cli::{AddLine, AddNew, SigningKeyArgs};
use crate::error::Error;
use crate::key::KeyLocation;
use crate::option::{KnownOptionName, OptionMap, OptionValue};
use crate::parse::{parse_custom_option, parse_line_entry};

/// A repository singing key.
pub enum SigningKey {
    /// The key is stored in a separate file.
    File { path: PathBuf },

    /// The key is inlined in the source file.
    Inline { value: OptionValue },
}

/// A repository source.
#[derive(Debug)]
pub struct RepoSource {
    name: String,
    options: OptionMap,
    key: Option<KeyLocation>,
    keyring_dir: PathBuf,
    inline_key: bool,
    overwrite: bool,
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

const SOURCES_DIR: &str = "/etc/apt/sources.list.d";

/// Parse the CLI args to determine where we're fetching the singing key from.
fn parse_key_args(args: &SigningKeyArgs) -> eyre::Result<Option<KeyLocation>> {
    Ok(match (&args.location.key, &args.keyserver) {
        (Some(key_location), Some(keyserver)) => Some(KeyLocation::Keyserver {
            fingerprint: key_location.to_string(),
            keyserver: keyserver.to_string(),
        }),
        (Some(key_location), None) => match Url::parse(key_location) {
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
    /// The path to install this repo source to.
    fn path(&self) -> PathBuf {
        [SOURCES_DIR, &format!("{}.sources", self.name)]
            .iter()
            .collect()
    }

    /// The path of a signing key for this repo source.
    fn key_path(&self) -> PathBuf {
        self.keyring_dir
            .join(format!("{}-archive-keyring.gpg", self.name))
    }

    /// Construct an instance from the CLI `args`.
    pub fn from_new_args(args: AddNew) -> eyre::Result<Self> {
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

        if let Some(description) = args.description.description {
            options.insert(KnownOptionName::RepolibName, description);
        }

        let key = parse_key_args(&args.key)?;
        let keyring_dir = Path::new(&args.key.keyring_dir).to_path_buf();

        Ok(Self {
            name: args.name.clone(),
            options,
            key,
            keyring_dir,
            inline_key: args.key.inline_key,
            overwrite: args.overwrite.overwrite,
        })
    }

    /// Construct an instance from the CLI `args`.
    pub fn from_add_args(args: AddLine) -> eyre::Result<Self> {
        let mut options = parse_line_entry(&args.line)
            .wrap_err("failed parsing single-line-style source entry")?;

        options.insert(KnownOptionName::Enabled, !args.disabled.disabled);

        if let Some(description) = args.description.description {
            options.insert(KnownOptionName::RepolibName, description);
        }

        let key = parse_key_args(&args.key)?;
        let keyring_dir = Path::new(&args.key.keyring_dir).to_path_buf();

        Ok(Self {
            name: args.name.clone(),
            options,
            key,
            keyring_dir,
            inline_key: args.key.inline_key,
            overwrite: args.overwrite.overwrite,
        })
    }

    /// Install the key for this repository source.
    pub fn install_key(&mut self) -> eyre::Result<()> {
        if let Some(key_location) = &self.key {
            let key = if self.inline_key {
                SigningKey::Inline {
                    value: key_location
                        .to_value()
                        .wrap_err("failed installing inline signing key")?,
                }
            } else {
                let path = self.key_path();

                key_location
                    .install(&path)
                    .wrap_err("failed installing singing key to file")?;

                SigningKey::File { path }
            };

            self.options.insert_key(key)?;
        }

        Ok(())
    }

    /// Open the repo source file, truncating if the user decided to overwrite.
    fn open_source_file(&self, path: &Path) -> eyre::Result<File> {
        if self.overwrite {
            match File::create(path) {
                Ok(file) => Ok(file),
                Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                    bail!(Error::PermissionDenied)
                }
                Err(err) => bail!(err),
            }
        } else {
            match OpenOptions::new().create_new(true).write(true).open(path) {
                Ok(file) => Ok(file),
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    bail!(Error::SourceFileAlreadyExists {
                        path: path.to_owned()
                    })
                }
                Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                    bail!(Error::PermissionDenied)
                }
                Err(err) => bail!(err),
            }
        }
    }

    /// Install this repo source as a file in deb822 format.
    pub fn install(&self) -> eyre::Result<()> {
        let mut file = self.open_source_file(&self.path())?;

        for (key, value) in self.options.options() {
            writeln!(&mut file, "{}: {}", key.to_deb822(), value.to_deb822())?;
        }

        file.flush()?;

        Ok(())
    }
}
