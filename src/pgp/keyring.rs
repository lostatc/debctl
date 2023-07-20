use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::process::{Command, Stdio};

use eyre::{bail, eyre, WrapErr};
use tempfile::NamedTempFile;

use crate::error::Error;

use super::key::{Key, KeyEncoding, KeyId};

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
        let output = Command::new("gpg")
            .arg("--no-default-keyring")
            .arg("--keyring")
            .arg(self.file.path().as_os_str())
            .arg("--keyserver")
            .arg(keyserver)
            .arg("--recv-keys")
            .arg(id.as_ref())
            .output()
            .wrap_err("failed to execute gpg command")?;

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
        let mut process = Command::new("gpg")
            .arg("--no-default-keyring")
            .arg("--keyring")
            .arg(self.file.path().as_os_str())
            .arg("--import")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .wrap_err("failed to execute gpg command")?;

        let mut stdin = process.stdin.take().unwrap();

        key.seek(SeekFrom::Start(0))?;

        io::copy(key, &mut stdin)?;

        drop(stdin);

        process.wait()?;

        Ok(KeyringKey { id: key.id()? })
    }

    /// Export a key from this keyring.
    pub fn export(&mut self, key: KeyringKey, encoding: KeyEncoding) -> eyre::Result<Key> {
        let mut process = Command::new("gpg")
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
            .spawn()
            .wrap_err("failed to execute gpg command")?;

        let mut stdout = process.stdout.take().unwrap();
        let mut stderr = process.stderr.take().unwrap();

        let stdout_handle = std::thread::spawn(move || -> eyre::Result<File> {
            let mut key_file = tempfile::tempfile()?;

            io::copy(&mut stdout, &mut key_file).wrap_err("filed reading stdout")?;

            Ok(key_file)
        });

        let stderr_handle = std::thread::spawn(move || -> eyre::Result<String> {
            let mut stderr_msg = String::new();

            stderr
                .read_to_string(&mut stderr_msg)
                .wrap_err("failed reading stderr")?;

            Ok(stderr_msg)
        });

        let status = process.wait()?;

        let key_file = stdout_handle
            .join()
            .expect("thread panicked writing stdout to file")?;
        let err_msg = stderr_handle
            .join()
            .expect("thread panicked reading stderr")?;

        if !status.success() {
            return Err(eyre!(err_msg).wrap_err("failed to export key from keyring"));
        }

        Ok(Key::new(key_file, encoding, Some(key.id)))
    }
}
