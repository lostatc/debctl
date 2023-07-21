use std::process::Stdio;

use eyre::{bail, WrapErr};
use tempfile::NamedTempFile;

use crate::error::Error;

use super::key::{Key, KeyEncoding, KeyId};
use super::stdio::{gpg_command, read_stderr, read_stdout, wait, write_stdin};

/// A PGP key in a keyring.
#[derive(Debug)]
pub struct KeyringKey {
    id: KeyId,
}

/// A PGP keyring.
#[derive(Debug)]
pub struct Keyring {
    file: NamedTempFile,
}

impl Keyring {
    /// Create a new empty keyring.
    pub fn new() -> eyre::Result<Self> {
        Ok(Self {
            file: NamedTempFile::new().wrap_err("failed to create temporary keyring file")?,
        })
    }

    /// Import a key into this keyring from a keyserver.
    pub fn recv_key(&mut self, keyserver: &str, id: KeyId) -> eyre::Result<KeyringKey> {
        let output = gpg_command()
            .arg("--no-default-keyring")
            .arg("--keyring")
            .arg(self.file.path().as_os_str())
            .arg("--keyserver")
            .arg(keyserver)
            .arg("--recv-keys")
            .arg(id.as_ref())
            .output()?;

        if !output.status.success() {
            bail!(Error::KeyserverFetchFailed {
                id: id.as_ref().to_string(),
                reason: String::from_utf8(output.stderr)
                    .wrap_err("failed to decode gpg command stderr")?,
            });
        }

        Ok(KeyringKey { id })
    }

    /// Import a key into this keyring.
    pub fn import(&mut self, key: &mut Key) -> eyre::Result<KeyringKey> {
        let mut process = gpg_command()
            .arg("--no-default-keyring")
            .arg("--keyring")
            .arg(self.file.path().as_os_str())
            .arg("--import")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;

        let stderr_handle = read_stderr(&mut process);

        write_stdin(&mut process, &mut key.as_ref())?;

        wait(process, stderr_handle)?;

        Ok(KeyringKey { id: key.id()? })
    }

    /// Export a key from this keyring.
    pub fn export(&mut self, key: KeyringKey, encoding: KeyEncoding) -> eyre::Result<Key> {
        let mut process = gpg_command()
            .arg("--no-default-keyring")
            .arg("--keyring")
            .arg(self.file.path().as_os_str())
            .args(match encoding {
                KeyEncoding::Binary => Vec::new(),
                KeyEncoding::Armored => vec!["--armor"],
            })
            .arg("--export")
            .arg(key.id.as_ref())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout_handle = read_stdout(&mut process);
        let stderr_handle = read_stderr(&mut process);

        wait(process, stderr_handle)?;

        let key_bytes = stdout_handle.join().unwrap()?;

        Ok(Key::new(key_bytes, encoding, Some(key.id)))
    }
}
