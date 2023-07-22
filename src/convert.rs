use std::path::PathBuf;

use eyre::{bail, WrapErr};

use crate::cli::{BackupArgs, Convert, ConvertDestArgs};
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

/// A converter for converting a repo source file.
#[derive(Debug)]
pub struct EntryConverter {
    options: Vec<OptionMap>,
    backup_mode: Option<BackupMode>,
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
    /// Construct an instance from CLI args.
    pub fn from_args(args: &Convert) -> eyre::Result<Self> {
        let in_file = args.dest.in_file()?;
        let out_file = args.dest.out_file()?;

        let options =
            parse_line_file(&in_file.path()).wrap_err("failed parsing source entry file")?;

        let backup_mode = BackupMode::from_args(&args.backup);

        Ok(EntryConverter {
            options,
            backup_mode,
            out_file,
        })
    }
}
