use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use eyre::{bail, eyre, WrapErr};

use crate::cli::Convert;
use crate::entry::{OverwriteAction, SourceEntry};
use crate::error::Error;
use crate::file::{SourceFile, SourceFileKind, SourceFilePath};
use crate::parse::{parse_line_file, ConvertedLineEntry};

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

/// A stream to read a source file from.
#[derive(Debug, Clone)]
enum InputStream {
    File(SourceFile),
    Stdin,
}

/// A stream to write a source file to.
#[derive(Debug, Clone)]
enum OutputStream {
    File(SourceFile),
    Stdout,
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
    in_file: InputStream,
    out_file: OutputStream,
}

impl Convert {
    /// The input source file.
    fn in_file(&self) -> eyre::Result<InputStream> {
        Ok(if let Some(name) = &self.name {
            InputStream::File(SourceFile {
                path: SourceFilePath::Installed {
                    name: name.to_owned(),
                },
                kind: SourceFileKind::OneLine,
            })
        } else if let Some(path) = &self.in_path {
            if path_is_stdio(path) {
                InputStream::Stdin
            } else {
                InputStream::File(SourceFile {
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
    fn out_file(&self) -> eyre::Result<OutputStream> {
        Ok(if let Some(name) = &self.name {
            OutputStream::File(SourceFile {
                path: SourceFilePath::Installed {
                    name: name.to_owned(),
                },
                kind: SourceFileKind::Deb822,
            })
        } else if let Some(path) = &self.out_path {
            if path_is_stdio(path) {
                OutputStream::Stdout
            } else {
                OutputStream::File(SourceFile {
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
    const BAKCKUP_EXT: &str = "bak";

    /// Construct an instance from CLI args.
    pub fn from_args(args: &Convert) -> eyre::Result<Self> {
        let in_file = args.in_file()?;
        let out_file = args.out_file()?;

        let mut source_stream: Box<dyn Read> = match args.in_file()? {
            InputStream::Stdin => Box::new(io::stdin()),
            InputStream::File(source_file) => match File::open(source_file.path()) {
                Ok(file) => Box::new(file),
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    bail!(Error::ConvertInFileNotFound {
                        path: source_file.path().into_owned()
                    })
                }
                Err(err) => return Err(err).wrap_err("failed opening source file"),
            },
        };

        let entries = match parse_line_file(&mut source_stream) {
            Ok(options) => options,
            Err(err) => match (in_file, err.downcast_ref::<io::Error>()) {
                (InputStream::File(source_file), Some(io_err))
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
                OutputStream::File(source_file) => Some(source_file.path().into_owned()),
                OutputStream::Stdout => None,
            },
            removed: match &self.in_file {
                InputStream::File(
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
            InputStream::File(source_file) => match &self.backup_mode {
                Some(BackupMode::Backup) => Some(BackupPlan {
                    original: source_file.path().into_owned(),
                    backup: PathBuf::from(format!(
                        "{}.{}",
                        source_file.path().as_os_str().to_string_lossy(),
                        Self::BAKCKUP_EXT,
                    )),
                }),
                Some(BackupMode::BackupTo { path }) => Some(BackupPlan {
                    original: source_file.path().into_owned(),
                    backup: path.to_owned(),
                }),
                None => None,
            },
            InputStream::Stdin => None,
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
            OutputStream::File(source_file) => source_file.path(),
            OutputStream::Stdout => return Ok(None),
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
            InputStream::File(
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
            Err(err) => bail!(err.wrap_err("failed opening `.sources` destination file")),
        };

        for line_entry in &self.entries {
            match line_entry {
                ConvertedLineEntry::Entry(options) => {
                    let entry = SourceEntry::new(options.clone(), None);

                    entry
                        .install_to(&mut output_file, OverwriteAction::Append)
                        .wrap_err("failed installing converted `.sources` source file")?;

                    // Adding a newline after stanzas ensures there's a blank line between the end
                    // of the stanza and any adjacent comments.
                    writeln!(&mut output_file)?;
                }
                ConvertedLineEntry::Comment(comment) => {
                    writeln!(&mut output_file, "# {}", comment)?;
                }
            }
        }

        if let OutputStream::Stdout = self.out_file {
            output_file.seek(SeekFrom::Start(0))?;
            io::copy(&mut output_file, &mut io::stdout())?;
        }

        self.remove_original()
            .wrap_err("failed deleting original `.list` source file")?;

        Ok(())
    }
}
