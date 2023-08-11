use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use eyre::{bail, WrapErr};

use crate::args::{AddArgs, NewArgs, OverwriteAction};
use crate::error::Error;
use crate::file::SourceFile;
use crate::key::{KeyDest, KeySource, SigningKey};
use crate::option::OptionMap;
use crate::pgp::GnupgClient;

/// A plan for how we will install the source entry.
///
/// Because a file might be created at the source file's path between this plan being generated and
/// the file actually being opened, we can't guarantee that this is exactly what will happen.
#[derive(Debug, Clone, Copy)]
pub enum InstallPlanAction {
    /// The source file was created.
    Create,

    /// The source file was overwritten.
    Overwrite,

    /// The source file was appended to.
    Append,
}

/// A plan for what will occur when we install the source entry.
///
/// The purpose of this type is to provide user-facing output explaining what will happen when we
/// install the source file, even without actually doing anything, such as when the user passes
/// `--dry-run`.
#[derive(Debug, Clone)]
pub struct InstallPlan {
    path: PathBuf,
    action: InstallPlanAction,
}

impl fmt::Display for InstallPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.action {
            InstallPlanAction::Create => f.write_fmt(format_args!(
                "Created new source file: {}\n",
                self.path.display()
            )),
            InstallPlanAction::Overwrite => f.write_fmt(format_args!(
                "Overwrote existing source file: {}\n",
                self.path.display()
            )),
            InstallPlanAction::Append => f.write_fmt(format_args!(
                "Appended new entry to existing source file: {}\n",
                self.path.display()
            )),
        }?;

        Ok(())
    }
}

impl InstallPlan {
    fn new(path: &Path, action: OverwriteAction) -> eyre::Result<Self> {
        Ok(Self {
            path: path.to_owned(),
            action: match (action, path.exists()) {
                (OverwriteAction::Overwrite, _) => InstallPlanAction::Overwrite,
                (OverwriteAction::Append, _) => InstallPlanAction::Append,
                (OverwriteAction::Fail, false) => InstallPlanAction::Create,
                (OverwriteAction::Fail, true) => bail!(Error::NewSourceFileAlreadyExists {
                    path: path.to_owned(),
                }),
            },
        })
    }
}

/// A repository source entry.
#[derive(Debug)]
pub struct SourceEntry {
    options: OptionMap,
    key: Option<KeySource>,
}

impl SourceEntry {
    /// Create a new instance.
    pub fn new(options: OptionMap, key: Option<KeySource>) -> Self {
        Self { options, key }
    }

    /// A plan for what installing this entry will do.
    pub fn plan(&self, file: &SourceFile, action: OverwriteAction) -> eyre::Result<InstallPlan> {
        InstallPlan::new(&file.path(), action)
    }

    /// Construct an instance from the CLI `args`.
    pub fn from_new(args: &NewArgs) -> eyre::Result<Self> {
        Ok(Self::new(args.options(), args.key().source.clone()))
    }

    /// Construct an instance from the CLI `args`.
    pub fn from_add(args: &AddArgs) -> eyre::Result<Self> {
        Ok(Self::new(args.options()?, args.key().source.to_owned()))
    }

    /// Install the key for this source entry.
    pub fn install_key(&mut self, client: &GnupgClient, dest: &KeyDest) -> eyre::Result<()> {
        if let Some(key_location) = &self.key {
            let key = match dest {
                KeyDest::File { path } => {
                    key_location
                        .install(client, path)
                        .wrap_err("failed installing signing key to file")?;

                    SigningKey::File { path: path.clone() }
                }
                KeyDest::Inline => SigningKey::Inline {
                    value: key_location
                        .to_value(client)
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
    pub fn install(&self, file: &SourceFile, action: OverwriteAction) -> eyre::Result<()> {
        let mut file = self.open_source_file(&file.path(), action)?;

        self.install_to(&mut file, action)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use rstest::*;
    use xpct::{be_err, be_existing_file, be_ok, equal, expect, have_len};

    use crate::cli;
    use crate::error::Error;
    use crate::file::{SourceFile, SourceFileKind, SourceFilePath};
    use crate::types::SourceType;

    use super::*;

    struct EntryParams {
        args: cli::New,
    }

    impl EntryParams {
        pub fn install(&self, file: &SourceFile, action: OverwriteAction) -> eyre::Result<()> {
            SourceEntry::from_new(&NewArgs::from_cli(self.args.clone())?)?.install(file, action)
        }
    }

    #[fixture]
    fn entry() -> EntryParams {
        EntryParams {
            args: cli::New {
                name: "myrepo".into(),
                uri: vec!["https://example.com".into()],
                description: cli::DescriptionArgs { description: None },
                suite: vec!["suite".into()],
                component: vec!["component".into()],
                kind: vec![SourceType::Deb],
                key: cli::SigningKeyArgs {
                    location: cli::KeySourceArgs {
                        key: None,
                        force_no_key: true,
                    },
                    keyserver: None,
                    destination: cli::KeyDestinationArgs {
                        key_path: None,
                        inline_key: false,
                    },
                },
                arch: Vec::new(),
                lang: Vec::new(),
                option: Vec::new(),
                force_literal_options: false,
                disabled: cli::DisabledArgs { disabled: false },
                overwrite: cli::OverwriteArgs {
                    overwrite: false,
                    append: false,
                },
            },
        }
    }

    #[rstest]
    fn installing_fails_when_output_file_already_exists(entry: EntryParams) -> eyre::Result<()> {
        let temp_file = tempfile::NamedTempFile::new()?;
        let dest_file = SourceFile {
            path: SourceFilePath::File {
                path: temp_file.path().to_owned(),
            },
            kind: SourceFileKind::Deb822,
        };

        expect!(entry.install(&dest_file, OverwriteAction::Fail))
            .to(be_err())
            .map(|err| err.downcast::<Error>())
            .to(be_ok())
            .to(equal(Error::NewSourceFileAlreadyExists {
                path: temp_file.path().to_owned(),
            }));

        expect!(temp_file.path()).to(be_existing_file());

        Ok(())
    }

    #[rstest]
    fn overwriting_succeeds_when_output_file_already_exists(
        entry: EntryParams,
    ) -> eyre::Result<()> {
        let temp_file = tempfile::NamedTempFile::new()?;
        let dest_file = SourceFile {
            path: SourceFilePath::File {
                path: temp_file.path().to_owned(),
            },
            kind: SourceFileKind::Deb822,
        };

        expect!(entry.install(&dest_file, OverwriteAction::Overwrite)).to(be_ok());
        expect!(temp_file.path()).to(be_existing_file());

        Ok(())
    }

    #[rstest]
    fn appending_succeeds_when_output_file_already_exists(entry: EntryParams) -> eyre::Result<()> {
        let temp_file = tempfile::NamedTempFile::new()?;
        let dest_file = SourceFile {
            path: SourceFilePath::File {
                path: temp_file.path().to_owned(),
            },
            kind: SourceFileKind::Deb822,
        };

        expect!(entry.install(&dest_file, OverwriteAction::Append)).to(be_ok());
        expect!(temp_file.path()).to(be_existing_file());

        Ok(())
    }

    #[rstest]
    fn installing_creates_output_file(entry: EntryParams) -> eyre::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let dest_path = temp_dir.path().join("myrepo.sources").to_owned();

        let dest_file = SourceFile {
            path: SourceFilePath::File {
                path: dest_path.clone(),
            },
            kind: SourceFileKind::Deb822,
        };

        expect!(entry.install(&dest_file, OverwriteAction::Fail)).to(be_ok());
        expect!(&dest_path).to(be_existing_file());

        Ok(())
    }

    #[rstest]
    fn overwriting_creates_output_file(entry: EntryParams) -> eyre::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let dest_path = temp_dir.path().join("myrepo.sources").to_owned();

        let dest_file = SourceFile {
            path: SourceFilePath::File {
                path: dest_path.clone(),
            },
            kind: SourceFileKind::Deb822,
        };

        expect!(entry.install(&dest_file, OverwriteAction::Overwrite)).to(be_ok());
        expect!(&dest_path).to(be_existing_file());

        Ok(())
    }

    #[rstest]
    fn appending_creates_output_file(entry: EntryParams) -> eyre::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let dest_path = temp_dir.path().join("myrepo.sources").to_owned();

        let dest_file = SourceFile {
            path: SourceFilePath::File {
                path: dest_path.clone(),
            },
            kind: SourceFileKind::Deb822,
        };

        expect!(entry.install(&dest_file, OverwriteAction::Append)).to(be_ok());
        expect!(&dest_path).to(be_existing_file());

        Ok(())
    }

    #[rstest]
    fn stanzas_are_separated_by_exactly_one_line(entry: EntryParams) -> eyre::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let dest_path = temp_dir.path().join("myrepo.sources").to_owned();

        let dest_file = SourceFile {
            path: SourceFilePath::File {
                path: dest_path.clone(),
            },
            kind: SourceFileKind::Deb822,
        };

        expect!(entry.install(&dest_file, OverwriteAction::Fail)).to(be_ok());
        expect!(entry.install(&dest_file, OverwriteAction::Append)).to(be_ok());

        let mut output_file = File::open(dest_path)?;

        let mut file_contents = String::new();
        output_file.read_to_string(&mut file_contents)?;

        expect!(file_contents)
            .map(|contents| {
                contents
                    .match_indices("\n\n")
                    .map(|(index, string)| (index, string.to_string()))
                    .collect::<Vec<_>>()
            })
            .to(have_len(1));

        Ok(())
    }
}
