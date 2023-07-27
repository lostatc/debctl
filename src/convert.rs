use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use eyre::{bail, eyre, WrapErr};

use crate::cli::Convert;
use crate::entry::{OverwriteAction, SourceEntry};
use crate::error::Error;
use crate::file::{SourceFile, SourceFileKind, SourceFilePath};
use crate::parse::{parse_line_file, ConvertedLineEntry, ParseLineFileOptions};

/// How to back up the original file when converting a repo source file.
#[derive(Debug)]
pub enum BackupMode {
    Backup,
    BackupTo { path: PathBuf },
}

impl BackupMode {
    /// Create an instance from CLI args.
    pub fn from_args(args: &Convert) -> Option<Self> {
        if args.backup {
            Some(Self::Backup)
        } else {
            args.backup_to.as_ref().map(|path| Self::BackupTo {
                path: path.to_owned(),
            })
        }
    }
}

/// Return whether this path is "-", meaning to read from stdin or write to stdout.
fn path_is_stdio(path: &Path) -> bool {
    path == Path::new("-")
}

/// A stream to read a source file from or write a source file to.
#[derive(Debug, Clone)]
enum IoStream {
    File(SourceFile),
    Stdio,
}

/// A plan for backing up a file.
struct BackupPlan {
    /// The original file path.
    original: PathBuf,

    /// The path to back it up to.
    backup: PathBuf,
}

/// A plan for what will occur when we convert the source entry.
///
/// The purpose of this type is to provide user-facing output explaining what will happen when we
/// convert the source file, even without actually doing anything, such as when the user passes
/// `--dry-run`.
#[derive(Debug, Clone)]
pub struct ConvertPlan {
    backed_up: Option<PathBuf>,
    created: Option<PathBuf>,
    removed: Option<PathBuf>,
}

impl fmt::Display for ConvertPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.backed_up {
            f.write_fmt(format_args!(
                "Backed up original source file: {}\n",
                path.display()
            ))?;
        }

        if let Some(path) = &self.created {
            f.write_fmt(format_args!(
                "Created new source file: {}\n",
                path.display(),
            ))?;
        }

        if let Some(path) = &self.removed {
            f.write_fmt(format_args!(
                "Removed original source file: {}\n",
                path.display(),
            ))?;
        }

        Ok(())
    }
}

/// A converter for converting a repo source file from the one-line syntax to the deb822 syntax.
#[derive(Debug)]
pub struct EntryConverter {
    entries: Vec<ConvertedLineEntry>,
    backup_mode: Option<BackupMode>,
    in_file: IoStream,
    out_file: IoStream,
}

impl Convert {
    /// The input source file.
    fn in_file(&self, sources_dir: PathBuf) -> eyre::Result<IoStream> {
        Ok(if let Some(name) = &self.name {
            IoStream::File(SourceFile {
                path: SourceFilePath::Installed {
                    name: name.to_owned(),
                    dir: sources_dir,
                },
                kind: SourceFileKind::OneLine,
            })
        } else if let Some(path) = &self.in_path {
            if path_is_stdio(path) {
                IoStream::Stdio
            } else {
                IoStream::File(SourceFile {
                    path: SourceFilePath::File {
                        path: path.to_owned(),
                    },
                    kind: SourceFileKind::OneLine,
                })
            }
        } else {
            bail!("unable to parse CLI arguments")
        })
    }

    /// The output source file.
    fn out_file(&self, sources_dir: PathBuf) -> eyre::Result<IoStream> {
        Ok(if let Some(name) = &self.name {
            IoStream::File(SourceFile {
                path: SourceFilePath::Installed {
                    name: name.to_owned(),
                    dir: sources_dir,
                },
                kind: SourceFileKind::Deb822,
            })
        } else if let Some(path) = &self.out_path {
            if path_is_stdio(path) {
                IoStream::Stdio
            } else {
                IoStream::File(SourceFile {
                    path: SourceFilePath::File {
                        path: path.to_owned(),
                    },
                    kind: SourceFileKind::Deb822,
                })
            }
        } else {
            bail!("unable to parse CLI arguments")
        })
    }
}

impl EntryConverter {
    pub const BACKUP_SUFFIX: &str = ".bak";

    /// Construct an instance from CLI args.
    pub fn from_args(args: &Convert, sources_dir: PathBuf) -> eyre::Result<Self> {
        let in_file = args.in_file(sources_dir.clone())?;
        let out_file = args.out_file(sources_dir.clone())?;

        let mut source_stream: Box<dyn Read> = match &in_file {
            IoStream::Stdio => Box::new(io::stdin()),
            IoStream::File(source_file) => match File::open(source_file.path()) {
                Ok(file) => Box::new(file),
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    bail!(Error::ConvertInFileNotFound {
                        path: source_file.path().into_owned()
                    })
                }
                Err(err) => return Err(err).wrap_err("failed opening source file"),
            },
        };

        let parse_options = ParseLineFileOptions {
            skip_comments: args.skip_comments,
            skip_disabled: args.skip_disabled,
        };

        let entries = match parse_line_file(&mut source_stream, &parse_options) {
            Ok(options) => options,
            Err(err) => match (in_file, err.downcast_ref::<io::Error>()) {
                (IoStream::File(source_file), Some(io_err))
                    if io_err.kind() == io::ErrorKind::NotFound =>
                {
                    bail!(Error::ConvertInFileNotFound {
                        path: source_file.path().into_owned(),
                    })
                }
                _ => bail!(err.wrap_err("failed to parse original source file")),
            },
        };

        let backup_mode = BackupMode::from_args(args);

        Ok(EntryConverter {
            entries,
            backup_mode,
            in_file,
            out_file,
        })
    }

    /// A plan for what converting the entry will do.
    pub fn plan(&self) -> ConvertPlan {
        ConvertPlan {
            backed_up: self.backup_plan().map(|plan| plan.backup),
            created: match &self.out_file {
                IoStream::File(source_file) => Some(source_file.path().into_owned()),
                IoStream::Stdio => None,
            },
            removed: match &self.in_file {
                IoStream::File(
                    path @ SourceFile {
                        path: SourceFilePath::Installed { .. },
                        ..
                    },
                ) => Some(path.path().into_owned()),
                _ => None,
            },
        }
    }

    /// Return the plan for backing up the source file.
    ///
    /// If this returns `None`, no backup is necessary.
    fn backup_plan(&self) -> Option<BackupPlan> {
        match &self.in_file {
            IoStream::File(source_file) => match &self.backup_mode {
                Some(BackupMode::Backup) => Some(BackupPlan {
                    original: source_file.path().into_owned(),
                    backup: PathBuf::from(format!(
                        "{}{}",
                        source_file.path().as_os_str().to_string_lossy(),
                        Self::BACKUP_SUFFIX,
                    )),
                }),
                Some(BackupMode::BackupTo { path }) => Some(BackupPlan {
                    original: source_file.path().into_owned(),
                    backup: path.to_owned(),
                }),
                None => None,
            },
            IoStream::Stdio => None,
        }
    }

    /// Open the file to back up the original source file to.
    fn open_backup_file(&self, path: &Path) -> eyre::Result<File> {
        let backup_file_result = OpenOptions::new().create_new(true).write(true).open(path);

        match backup_file_result {
            Ok(file) => Ok(file),
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                Err(eyre!(Error::PermissionDenied))
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                Err(eyre!(Error::ConvertBackupAlreadyExists {
                    path: path.to_owned()
                }))
            }
            Err(err) => Err(err).wrap_err("failed opening backup source file"),
        }
    }

    /// Backup the original source file.
    fn backup_original(&self) -> eyre::Result<()> {
        let backup_plan = match self.backup_plan() {
            Some(plan) => plan,
            None => return Ok(()),
        };

        let mut backup_file = self.open_backup_file(&backup_plan.backup)?;

        let mut source_file =
            File::open(&backup_plan.original).wrap_err("failed opening original source file")?;

        io::copy(&mut source_file, &mut backup_file)
            .wrap_err("failed copying bytes from original source file to backup file")?;

        Ok(())
    }

    /// Open the destination file for the converted source file.
    ///
    /// If this returns `None`, then we're writing to stdout.
    fn open_dest_file(&self) -> eyre::Result<Option<File>> {
        let out_path = match &self.out_file {
            IoStream::File(source_file) => source_file.path(),
            IoStream::Stdio => return Ok(None),
        };

        let result = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&out_path);

        match result {
            Ok(file) => Ok(Some(file)),
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                Err(eyre!(Error::PermissionDenied))
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                Err(eyre!(Error::ConvertOutFileAlreadyExists {
                    path: out_path.into_owned(),
                }))
            }
            Err(err) => Err(eyre!(err)),
        }
    }

    /// Delete the original source file.
    fn remove_original(&self) -> eyre::Result<()> {
        match &self.in_file {
            // We only delete the original file if we're installing the new file directly into the
            // apt sources directory.
            IoStream::File(
                path @ SourceFile {
                    path: SourceFilePath::Installed { .. },
                    ..
                },
            ) => match fs::remove_file(path.path()) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                    Err(eyre!(Error::PermissionDenied))
                }
                Err(err) => Err(eyre!(err)),
            },
            _ => Ok(()),
        }
    }

    /// Convert the source entry.
    pub fn convert(&self) -> eyre::Result<()> {
        self.backup_original()
            .wrap_err("failed to create backup of original `.list` source file")?;

        let mut output_file = match self.open_dest_file() {
            Ok(Some(file)) => file,
            Ok(None) => tempfile::tempfile()?,
            Err(err) => bail!(err.wrap_err("failed opening destination source file")),
        };

        for (entry_index, line_entry) in self.entries.iter().enumerate() {
            match line_entry {
                ConvertedLineEntry::Entry(options) => {
                    let entry = SourceEntry::new(options.clone(), None);

                    entry
                        .install_to(&mut output_file, OverwriteAction::Append)
                        .wrap_err("failed installing converted `.sources` source file")?;

                    // Adding a newline after stanzas ensures there's a blank line between the end
                    // of the stanza and any adjacent comments. But don't add a trailing newline at
                    // the end of the file.
                    if entry_index < self.entries.len() - 1 {
                        writeln!(&mut output_file)?;
                    }
                }
                ConvertedLineEntry::Comment(comment) => {
                    writeln!(&mut output_file, "# {}", comment)?;
                }
            }
        }

        if let IoStream::Stdio = self.out_file {
            output_file.seek(SeekFrom::Start(0))?;
            io::copy(&mut output_file, &mut io::stdout())?;
        }

        self.remove_original()
            .wrap_err("failed deleting original `.list` source file")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rstest::*;
    use xpct::{be_err, be_existing_file, be_ok, equal, expect};

    use crate::cli;

    use super::*;

    const REPO_NAME: &str = "myrepo";

    struct ConverterParams {
        name: String,
        sources_dir: tempfile::TempDir,
        temp_dir: tempfile::TempDir,
        source_file: PathBuf,
        dest_file: PathBuf,
        args: cli::Convert,
    }

    impl ConverterParams {
        pub fn convert(&self) -> eyre::Result<()> {
            EntryConverter::from_args(&self.args, self.sources_dir.path().to_owned())?.convert()?;

            Ok(())
        }
    }

    #[fixture]
    fn by_name() -> eyre::Result<ConverterParams> {
        let sources_dir = tempfile::tempdir()?;
        let temp_dir = tempfile::tempdir()?;

        let source_file = sources_dir.path().join(format!("{REPO_NAME}.list"));
        let dest_file = sources_dir.path().join(format!("{REPO_NAME}.sources"));

        File::create(&source_file)?;

        let args = cli::Convert {
            name: Some(REPO_NAME.into()),
            in_path: None,
            out_path: None,
            backup: false,
            backup_to: None,
            skip_comments: false,
            skip_disabled: false,
        };

        Ok(ConverterParams {
            name: REPO_NAME.to_string(),
            sources_dir,
            temp_dir,
            source_file,
            dest_file,
            args,
        })
    }

    #[fixture]
    fn by_path() -> eyre::Result<ConverterParams> {
        let sources_dir = tempfile::tempdir()?;
        let temp_dir = tempfile::tempdir()?;

        let source_file = temp_dir.path().join(format!("{REPO_NAME}.list"));
        let dest_file = temp_dir.path().join(format!("{REPO_NAME}.sources"));

        File::create(&source_file)?;

        let args = cli::Convert {
            name: None,
            in_path: Some(source_file.clone()),
            out_path: Some(dest_file.clone()),
            backup: false,
            backup_to: None,
            skip_comments: false,
            skip_disabled: false,
        };

        Ok(ConverterParams {
            name: REPO_NAME.to_string(),
            sources_dir,
            temp_dir,
            source_file,
            dest_file,
            args,
        })
    }

    #[rstest]
    fn new_file_is_created_by_name(by_name: eyre::Result<ConverterParams>) -> eyre::Result<()> {
        let params = by_name?;

        params.convert()?;

        expect!(params.dest_file).to(be_existing_file());

        Ok(())
    }

    #[rstest]
    fn new_file_is_created_by_path(by_path: eyre::Result<ConverterParams>) -> eyre::Result<()> {
        let params = by_path?;

        params.convert()?;

        expect!(params.dest_file).to(be_existing_file());

        Ok(())
    }

    #[rstest]
    fn original_file_is_deleted_by_name(
        by_name: eyre::Result<ConverterParams>,
    ) -> eyre::Result<()> {
        let params = by_name?;

        params.convert()?;

        expect!(params.source_file).to_not(be_existing_file());

        Ok(())
    }

    #[rstest]
    fn original_file_is_not_deleted_by_path(
        by_path: eyre::Result<ConverterParams>,
    ) -> eyre::Result<()> {
        let params = by_path?;

        params.convert()?;

        expect!(params.source_file).to(be_existing_file());

        Ok(())
    }

    #[rstest]
    fn original_file_is_backed_up(by_name: eyre::Result<ConverterParams>) -> eyre::Result<()> {
        let mut params = by_name?;
        let backup_file = params.sources_dir.path().join(format!(
            "{}.list{}",
            params.name,
            EntryConverter::BACKUP_SUFFIX
        ));

        params.args.backup = true;

        params.convert()?;

        expect!(backup_file).to(be_existing_file());

        Ok(())
    }

    #[rstest]
    fn original_file_is_backed_up_to(by_name: eyre::Result<ConverterParams>) -> eyre::Result<()> {
        let mut params = by_name?;
        let backup_file = params.temp_dir.path().join("my-backup-file.list");

        params.args.backup_to = Some(backup_file.clone());

        params.convert()?;

        expect!(backup_file).to(be_existing_file());

        Ok(())
    }

    #[rstest]
    fn fails_when_input_file_does_not_exist_by_name(
        by_name: eyre::Result<ConverterParams>,
    ) -> eyre::Result<()> {
        let params = by_name?;

        fs::remove_file(&params.source_file)?;

        expect!(params.convert())
            .to(be_err())
            .map(|err| err.downcast::<Error>())
            .to(be_ok())
            .to(equal(Error::ConvertInFileNotFound {
                path: params.source_file,
            }));

        Ok(())
    }

    #[rstest]
    fn fails_when_input_file_does_not_exist_by_path(
        by_path: eyre::Result<ConverterParams>,
    ) -> eyre::Result<()> {
        let params = by_path?;

        fs::remove_file(&params.source_file)?;

        expect!(params.convert())
            .to(be_err())
            .map(|err| err.downcast::<Error>())
            .to(be_ok())
            .to(equal(Error::ConvertInFileNotFound {
                path: params.source_file,
            }));

        Ok(())
    }

    #[rstest]
    fn fails_when_output_file_already_exists_by_name(
        by_name: eyre::Result<ConverterParams>,
    ) -> eyre::Result<()> {
        let params = by_name?;

        File::create(&params.dest_file)?;

        expect!(params.convert())
            .to(be_err())
            .map(|err| err.downcast::<Error>())
            .to(be_ok())
            .to(equal(Error::ConvertOutFileAlreadyExists {
                path: params.dest_file,
            }));

        Ok(())
    }

    #[rstest]
    fn fails_when_output_file_already_exists_by_path(
        by_path: eyre::Result<ConverterParams>,
    ) -> eyre::Result<()> {
        let params = by_path?;

        File::create(&params.dest_file)?;

        expect!(params.convert())
            .to(be_err())
            .map(|err| err.downcast::<Error>())
            .to(be_ok())
            .to(equal(Error::ConvertOutFileAlreadyExists {
                path: params.dest_file,
            }));

        Ok(())
    }

    #[rstest]
    fn fails_when_backup_file_already_exists(
        by_name: eyre::Result<ConverterParams>,
    ) -> eyre::Result<()> {
        let mut params = by_name?;

        let backup_file = params.sources_dir.path().join(format!(
            "{}.list{}",
            params.name,
            EntryConverter::BACKUP_SUFFIX
        ));

        File::create(&backup_file)?;

        params.args.backup = true;

        expect!(params.convert())
            .to(be_err())
            .map(|err| err.downcast::<Error>())
            .to(be_ok())
            .to(equal(Error::ConvertBackupAlreadyExists {
                path: backup_file,
            }));

        Ok(())
    }

    #[rstest]
    fn fails_when_backup_to_file_already_exists(
        by_name: eyre::Result<ConverterParams>,
    ) -> eyre::Result<()> {
        let mut params = by_name?;

        let backup_file = params.temp_dir.path().join(format!(
            "{}.list{}",
            params.name,
            EntryConverter::BACKUP_SUFFIX
        ));

        File::create(&backup_file)?;

        params.args.backup_to = Some(backup_file.clone());

        expect!(params.convert())
            .to(be_err())
            .map(|err| err.downcast::<Error>())
            .to(be_ok())
            .to(equal(Error::ConvertBackupAlreadyExists {
                path: backup_file,
            }));

        Ok(())
    }
}
