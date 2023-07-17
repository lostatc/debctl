use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use eyre::bail;

use crate::error::Error;
use crate::option::{KnownSourceOption, OptionPair, SourceOption};
use crate::source::RepoSource;

impl RepoSource {
    /// Convert this repo source to a list of key-value pairs.
    fn to_options(&self) -> Vec<OptionPair> {
        use KnownSourceOption::*;

        let mut known_options = Vec::new();

        if !self.uris.is_empty() {
            known_options.push((Uris, self.uris.join(" ")));
        }

        if let Some(description) = self.description.clone() {
            known_options.push((RepolibName, description));
        }

        if !self.suites.is_empty() {
            known_options.push((Suites, self.suites.join(" ")));
        }

        if !self.components.is_empty() {
            known_options.push((Components, self.components.join(" ")));
        }

        if !self.kinds.is_empty() {
            known_options.push((
                Types,
                self.kinds
                    .iter()
                    .map(AsRef::as_ref)
                    .collect::<Vec<_>>()
                    .join(" "),
            ));
        }

        if self.key.is_some() {
            known_options.push((SignedBy, self.key_path().to_string_lossy().to_string()));
        }

        if !self.architectures.is_empty() {
            known_options.push((Architectures, self.architectures.join(" ")));
        }

        if !self.languages.is_empty() {
            known_options.push((Languages, self.languages.join(" ")));
        }

        if !self.enabled {
            known_options.push((Enabled, String::from("no")));
        }

        let mut options = known_options
            .into_iter()
            .map(|(key, value)| (SourceOption::Known(key), value))
            .collect::<Vec<_>>();

        options.extend_from_slice(&self.extra);

        options
    }

    /// Write this repo source to a file at `path` in deb822 format.
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
