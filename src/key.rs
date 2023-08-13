use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

use eyre::{bail, WrapErr};
use reqwest::Url;

use crate::error::Error;
use crate::option::OptionValue;
use crate::pgp::{Key, KeyEncoding, KeyId, PgpClient};

/// The location to install a signing key to.
#[derive(Debug, Clone)]
pub enum KeyDest {
    /// Inline it into the source entry.
    Inline,

    /// Install it to a separate file.
    File { path: PathBuf },
}

/// A location to acquire a signing key from.
#[derive(Debug, Clone)]
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
    fn get_key(&self, client: &dyn PgpClient, encoding: KeyEncoding) -> eyre::Result<Key> {
        match self {
            Self::Download { url } => Ok(client
                .download_key(url, encoding)
                .wrap_err("failed downloading signing key")?),
            Self::File { path } => Ok(client
                .read_key(path, encoding)
                .wrap_err("failed getting signing key from file")?),
            Self::Keyserver { id, keyserver } => Ok(client
                .recv_key(keyserver, KeyId::new(id.to_string()), encoding)
                .wrap_err("failed getting signing key from keyserver")?),
        }
    }

    /// Install the signing key at this location to `dest`.
    pub fn install(&self, client: &dyn PgpClient, dest: &Path) -> eyre::Result<()> {
        let key = self
            .get_key(client, KeyEncoding::Binary)
            .wrap_err("failed getting signing key")?;

        let mut dest_file = open_key_destination(dest)?;

        io::copy(&mut key.as_ref(), &mut dest_file)
            .wrap_err("failed copying key to destination")?;

        Ok(())
    }

    /// Get the key at this location as an option value.
    pub fn to_value(&self, client: &dyn PgpClient) -> eyre::Result<OptionValue> {
        let key = self
            .get_key(client, KeyEncoding::Armored)
            .wrap_err("failed getting signing key")?;

        Ok(OptionValue::Multiline(
            BufReader::new(key.as_ref())
                .lines()
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}

/// A repository signing key.
#[derive(Debug)]
pub enum SigningKey {
    /// The key is stored in a separate file.
    File { path: PathBuf },

    /// The key is inlined in the source entry.
    Inline { value: OptionValue },
}
