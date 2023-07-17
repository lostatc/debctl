use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use eyre::bail;

use crate::error::Error;
use crate::source::RepoSource;

impl RepoSource {
    pub fn write(&self, path: &Path) -> eyre::Result<()> {
        let mut file = if self.overwrite {
            File::create(path)?
        } else {
            match OpenOptions::new().create_new(true).write(true).open(path) {
                Ok(file) => file,
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    bail!(Error::SourceFileAlreadyExists {
                        path: path.to_owned()
                    })
                }
                Err(err) => bail!(err),
            }
        };

        for (key, value) in self.to_options() {
            writeln!(&mut file, "{}: {}", key.into_deb822(), value)?;
        }

        file.flush()?;

        Ok(())
    }
}
