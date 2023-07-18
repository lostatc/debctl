use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use eyre::bail;

use crate::error::Error;
use crate::source::RepoSource;

impl RepoSource {
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

    /// Write this repo source to a file at `path` in deb822 format.
    pub fn install(&self, path: &Path) -> eyre::Result<()> {
        let mut file = self.open_source_file(path)?;

        for (key, value) in self.options.options() {
            writeln!(&mut file, "{}: {}", key.to_deb822(), value.to_deb822())?;
        }

        file.flush()?;

        Ok(())
    }
}
