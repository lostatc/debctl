use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

use eyre::{bail, WrapErr};
use reqwest::Url;

use crate::error::Error;
use crate::option::OptionValue;
use crate::pgp::{Key, KeyEncoding, KeyId, Keyring};

/// A location to acquire a public singing key from.
#[derive(Debug)]
pub enum KeySource {
    /// Download the key from a URL.
    Download { url: Url },

    /// Copy the file from a path.
    File { path: PathBuf },

    /// Fetch the key from a keyserver.
    Keyserver { id: String, keyserver: String },
}

/// Ensure the given directory exists.
fn ensure_dir_exists(dir: &Path) -> eyre::Result<()> {
    match fs::create_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => bail!(Error::PermissionDenied),
        Err(err) => Err(err).wrap_err("failed creating directory"),
    }
}

/// Open the destination file to install the key to.
fn open_key_destination(path: &Path) -> eyre::Result<File> {
    if let Some(keyring_dir) = path.parent() {
        ensure_dir_exists(keyring_dir).wrap_err("failed creating keyring directory")?;
    }

    match File::create(path) {
        Ok(file) => Ok(file),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => bail!(Error::PermissionDenied),
        Err(err) => Err(err).wrap_err("failed opening destination key file for writing")?,
    }
}

impl KeySource {
    /// Get signing key at this location.
    fn get_key(&self) -> eyre::Result<Key> {
        match self {
            Self::Download { url } => {
                Ok(Key::from_url(url).wrap_err("failed downloading signing key")?)
            }
            Self::File { path } => {
                Ok(Key::from_file(path).wrap_err("failed getting signing key from file")?)
            }
            Self::Keyserver { id, keyserver } => {
                let mut keyring = Keyring::new().wrap_err("failed creating keyring")?;

                let keyring_key = keyring
                    .recv_key(keyserver, KeyId::new(id.to_string()))
                    .wrap_err("failed getting signing key from keyserver")?;

                Ok(keyring
                    .export(keyring_key, KeyEncoding::Binary)
                    .wrap_err("failed exporting signing key from keyring")?)
            }
        }
    }

    /// Install the signing key at this location to `dest`.
    pub fn install(&self, dest: &Path) -> eyre::Result<()> {
        let key = self.get_key().wrap_err("failed getting signing key")?;

        let dearmored_key = key.dearmor().wrap_err("failed dearmoring signing key")?;

        let mut dest_file = open_key_destination(dest)?;

        io::copy(&mut dearmored_key.as_ref(), &mut dest_file)
            .wrap_err("failed copying key to destination")?;

        Ok(())
    }

    /// Get the key at this location as an option value.
    pub fn to_value(&self) -> eyre::Result<OptionValue> {
        let key = self.get_key().wrap_err("failed getting signing key")?;

        let armored_key = key.enarmor().wrap_err("failed armoring signing key")?;

        Ok(OptionValue::Multiline(
            BufReader::new(armored_key.as_ref())
                .lines()
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}
