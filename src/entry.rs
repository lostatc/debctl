use std::borrow::Cow;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::Command;

use eyre::{bail, eyre, WrapErr};
use reqwest::Url;

use crate::cli::{Add, New, OverwriteArgs, SigningKeyArgs};
use crate::error::Error;
use crate::file::{SourceFile, SourceFileKind, SourceFilePath};
use crate::key::{KeyDestination, KeySource, SigningKey};
use crate::option::{KnownOptionName, OptionMap};
use crate::parse::{parse_custom_option, parse_line_entry};

/// What to do if a repo source file already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverwriteAction {
    Overwrite,
    Append,
    Fail,
}

impl OverwriteArgs {
    /// What to do if a repo source file already exists.
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

/// What will probably happen when we install the source file.
///
/// This is used so that we can provide useful output when the user passes `--dry-run`. Because the
/// source file might be created between this plan being generated and the file actually being
/// opened, we can't guarantee that this is exactly what will happen.
#[derive(Debug, Clone, Copy)]
pub enum InstallPlan {
    /// The source file was created.
    Create,

    /// The source file was overwritten.
    Overwrite,

    /// The source file was appended to.
    Append,
}

impl InstallPlan {
    pub fn new(path: &Path, action: OverwriteAction) -> eyre::Result<Self> {
        if !path.exists() {
            return Ok(Self::Create);
        }

        match action {
            OverwriteAction::Overwrite => Ok(Self::Overwrite),
            OverwriteAction::Append => Ok(Self::Append),
            OverwriteAction::Fail => Err(eyre!(Error::NewSourceFileAlreadyExists {
                path: path.to_owned(),
            })),
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
    /// Create a new instance.
    pub fn new(file: SourceFile, options: OptionMap, key: Option<KeySource>) -> Self {
        Self { file, options, key }
    }

    /// The path this entry will be installed to.
    pub fn path(&self) -> Cow<'_, Path> {
        self.file.path()
    }

    /// Construct an instance from the CLI `args`.
    pub fn from_new_args(args: &New) -> eyre::Result<Self> {
        let mut options = args
            .option
            .iter()
            .map(|option| parse_custom_option(option, args.force_literal_options))
            .collect::<Result<OptionMap, _>>()?;

        options.insert(KnownOptionName::Uris, args.uri.clone());

        options.insert(KnownOptionName::Types, args.kind.clone());

        options.insert(KnownOptionName::Components, args.component.clone());

        options.insert(KnownOptionName::Architectures, args.arch.clone());

        options.insert(KnownOptionName::Languages, args.lang.clone());

        options.insert(KnownOptionName::Enabled, !args.disabled.disabled);

        options.insert_or_else(
            KnownOptionName::Suites,
            args.suite.to_owned(),
            get_current_codename,
        )?;

        if let Some(description) = &args.description.description {
            options.insert(KnownOptionName::RepolibName, description.clone());
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
    pub fn from_add_args(args: &Add) -> eyre::Result<Self> {
        let mut options = parse_line_entry(&args.line)
            .wrap_err("failed parsing single-line-style source entry")?;

        options.insert(KnownOptionName::Enabled, !args.disabled.disabled);

        if let Some(description) = &args.description.description {
            options.insert(KnownOptionName::RepolibName, description.clone());
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
    pub fn install_key(&mut self, dest: &KeyDestination) -> eyre::Result<()> {
        if let Some(key_location) = &self.key {
            let key = match dest {
                KeyDestination::File { path } => {
                    key_location
                        .install(path)
                        .wrap_err("failed installing signing key to file")?;

                    SigningKey::File { path: path.clone() }
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

    fn write_options(&self, mut dest: impl Write) -> eyre::Result<()> {
        for (key, value) in self.options.options() {
            writeln!(&mut dest, "{}: {}", key.to_deb822(), value.to_deb822())
                .wrap_err("failed writing option to source file")?;
        }

        Ok(())
    }

    /// Install this source entry to the given file in deb822 format.
    pub fn install_to(&self, mut file: &mut File, action: OverwriteAction) -> eyre::Result<()> {
        if action == OverwriteAction::Append {
            file.seek(SeekFrom::Start(0))?;

            let buf_reader = BufReader::new(&mut file);
            let mut last_line: Option<String> = None;

            for line in buf_reader.lines() {
                last_line = Some(line.wrap_err("failed reading from source file")?);
            }

            file.seek(SeekFrom::End(0))?;

            // Stanzas in a deb822 file must have a blank line between them, but we don't want to
            // add unnecessary blank lines if the file already ends with one.
            if let Some(line) = last_line {
                if !line.trim().is_empty() {
                    writeln!(&mut file)?;
                }
            }
        }

        self.write_options(&mut file)?;

        Ok(())
    }

    /// Install this source entry as a file in deb822 format.
    pub fn install(&self, action: OverwriteAction) -> eyre::Result<()> {
        let mut file = self.open_source_file(&self.file.path(), action)?;

        self.install_to(&mut file, action)
    }
}
