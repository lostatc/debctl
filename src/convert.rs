use std::borrow::Cow;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use eyre::{bail, eyre, WrapErr};

use crate::cli::Convert;
use crate::entry::{OverwriteAction, SourceEntry};
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

fn path_is_stdio(path: &Path) -> bool {
    path == Path::new("-")
}

/// A converter for converting a repo source file from the single-line syntax to the deb822 syntax.
#[derive(Debug)]
pub struct EntryConverter {
    options: Vec<OptionMap>,
    backup_mode: Option<BackupMode>,
    in_file: SourceFile,
    out_file: SourceFile,
    remove_original: bool,
}

impl Convert {
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
    const BAKCKUP_EXT: &str = "bak";

    /// Construct an instance from CLI args.
    pub fn from_args(args: &Convert) -> eyre::Result<Self> {
        let in_file = args.in_file()?;
        let out_file = args.out_file()?;

        let in_path = in_file.path();
        let input_is_stdin = path_is_stdio(&in_path);

        let mut source_file: Box<dyn Read> = if input_is_stdin {
            Box::new(io::stdin())
        } else {
            Box::new(File::open(&in_path).wrap_err("failed to open source file")?)
        };

        let options = match parse_line_file(&mut source_file) {
            Ok(options) => options,
            Err(err) => match err.downcast_ref::<io::Error>() {
                Some(io_err) if io_err.kind() == io::ErrorKind::NotFound => {
                    bail!(Error::ConvertInFileNotFound {
                        path: in_path.into_owned(),
                    })
                }
                _ => bail!(err.wrap_err("failed to parse original source file")),
            },
        };

        let backup_mode = BackupMode::from_args(args);

        Ok(EntryConverter {
            options,
            backup_mode,
            in_file,
            out_file,
            remove_original: args.name.is_some() && !input_is_stdin,
        })
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
        let in_path = self.in_file.path();

        let backup_path = match &self.backup_mode {
            Some(BackupMode::Backup) => Cow::Owned(PathBuf::from(
                format!(
                    "{}.{}",
                    in_path.as_os_str().to_string_lossy(),
                    Self::BAKCKUP_EXT
                )
                .to_string(),
            )),
            Some(BackupMode::BackupTo { path }) => Cow::Borrowed(path),
            None => return Ok(()),
        };

        let mut backup_file = self.open_backup_file(backup_path.as_ref())?;

        let mut source_file =
            File::open(&in_path).wrap_err("failed opening original source file")?;

        io::copy(&mut source_file, &mut backup_file)
            .wrap_err("failed copying bytes from original source file to backup file")?;

        Ok(())
    }

    /// Open the destination file for the converted source file.
    fn open_dest_file(&self) -> eyre::Result<File> {
        let result = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(self.out_file.path());

        match result {
            Ok(file) => Ok(file),
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                Err(eyre!(Error::PermissionDenied))
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                Err(eyre!(Error::ConvertOutFileAlreadyExists {
                    path: self.out_file.path().into_owned(),
                }))
            }
            Err(err) => Err(eyre!(err)),
        }
    }

    /// Delete the original source file.
    fn remove_original(&self) -> eyre::Result<()> {
        if !self.remove_original {
            return Ok(());
        }

        match fs::remove_file(self.in_file.path()) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                Err(eyre!(Error::PermissionDenied))
            }
            Err(err) => Err(eyre!(err)),
        }
    }

    /// Convert the source entry.
    pub fn convert(self) -> eyre::Result<()> {
        self.backup_original()
            .wrap_err("failed to create backup of original `.list` source file")?;

        let out_path = self.out_file.path();

        let mut output_file = if path_is_stdio(&out_path) {
            tempfile::tempfile()?
        } else {
            self.open_dest_file()
                .wrap_err("failed opening `.sources` destination file")?
        };

        for options in &self.options {
            let entry = SourceEntry::new(self.out_file.clone(), options.clone(), None);

            entry
                .install_to(&mut output_file, OverwriteAction::Append)
                .wrap_err("failed installing converted `.sources` source file")?;
        }

        if path_is_stdio(&out_path) {
            output_file.seek(SeekFrom::Start(0))?;
            io::copy(&mut output_file, &mut io::stdout())?;
        }

        self.remove_original()
            .wrap_err("failed deleting original `.list` source file")?;

        Ok(())
    }
}
