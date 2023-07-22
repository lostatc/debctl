use std::borrow::Cow;
use std::fs;
use std::io;
use std::path::PathBuf;

use eyre::{bail, WrapErr};

use crate::cli::{BackupArgs, Convert, ConvertDestArgs};
use crate::error::Error;
use crate::file::{SourceFile, SourceFileKind, SourceFilePath};
use crate::option::OptionMap;
use crate::parse::parse_line_file;

/// How to back up the original file when converting a repo source file.
#[derive(Debug)]
enum BackupMode {
    Backup,
    BackupTo { path: PathBuf },
}

impl BackupMode {
    /// Create an instance from CLI args.
    pub fn from_args(args: &BackupArgs) -> Option<Self> {
        if args.backup {
            Some(Self::Backup)
        } else {
            args.backup_to.as_ref().map(|path| Self::BackupTo {
                path: path.to_owned(),
            })
        }
    }
}

/// A converter for converting a repo source file from the single-line syntax to the deb822 syntax.
#[derive(Debug)]
pub struct EntryConverter {
    options: Vec<OptionMap>,
    backup_mode: Option<BackupMode>,
    in_file: SourceFile,
    out_file: SourceFile,
}

impl ConvertDestArgs {
    /// The input source file.
    pub fn in_file(&self) -> eyre::Result<SourceFile> {
        if let Some(name) = &self.name {
            Ok(SourceFile {
                path: SourceFilePath::Installed {
                    name: name.to_owned(),
                },
                kind: SourceFileKind::SingleLine,
            })
        } else if let Some(path) = &self.in_path {
            Ok(SourceFile {
                path: SourceFilePath::File {
                    path: path.to_owned(),
                },
                kind: SourceFileKind::SingleLine,
            })
        } else {
            bail!("unable to parse CLI arguments")
        }
    }

    /// The output source file.
    pub fn out_file(&self) -> eyre::Result<SourceFile> {
        if let Some(name) = &self.name {
            Ok(SourceFile {
                path: SourceFilePath::Installed {
                    name: name.to_owned(),
                },
                kind: SourceFileKind::Deb822,
            })
        } else if let Some(path) = &self.out_path {
            Ok(SourceFile {
                path: SourceFilePath::File {
                    path: path.to_owned(),
                },
                kind: SourceFileKind::Deb822,
            })
        } else {
            bail!("unable to parse CLI arguments")
        }
    }
}

impl EntryConverter {
    const BAKCKUP_EXT: &str = ".bak";

    /// Construct an instance from CLI args.
    pub fn from_args(args: &Convert) -> eyre::Result<Self> {
        let in_file = args.dest.in_file()?;
        let out_file = args.dest.out_file()?;

        let in_path = in_file.path();

        let options = match parse_line_file(&in_path) {
            Ok(options) => options,
            Err(err) => match err.downcast_ref::<io::Error>() {
                Some(io_err) if io_err.kind() == io::ErrorKind::NotFound => {
                    bail!(Error::SourceFileNotFound {
                        path: in_path.into_owned(),
                    })
                }
                _ => bail!(err.wrap_err("failed to parse original source file")),
            },
        };

        let backup_mode = BackupMode::from_args(&args.backup);

        Ok(EntryConverter {
            options,
            backup_mode,
            in_file,
            out_file,
        })
    }

    /// Backup the original source file.
    fn backup_original(&self) -> eyre::Result<()> {
        let in_path = self.in_file.path();

        let backup_path = match &self.backup_mode {
            Some(BackupMode::Backup) => Cow::Owned(in_path.join(Self::BAKCKUP_EXT)),
            Some(BackupMode::BackupTo { path }) => Cow::Borrowed(path),
            None => return Ok(()),
        };

        fs::copy(&in_path, backup_path.as_ref())
            .wrap_err("failed copying source file to backup path")?;

        Ok(())
    }

    /// Convert the source entry.
    pub fn convert(&self) -> eyre::Result<()> {
        self.backup_original()
            .wrap_err("failed to create backup of original source file")?;

        todo!()
    }
}
