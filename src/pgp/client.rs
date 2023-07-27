use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use eyre::{bail, eyre, WrapErr};
use regex::Regex;
use reqwest::Url;

use crate::error::Error;
use crate::stdio::write_stdin;

use super::net::download_file;
use super::{Key, KeyEncoding};

static PGP_ARMOR_REGEX: OnceLock<Regex> = OnceLock::new();

/// A regex which matches the first line of an ASCII-armored public PGP key.
fn pgp_armor_regex() -> &'static Regex {
    PGP_ARMOR_REGEX.get_or_init(|| {
        Regex::new(r#"^\s*-----\s*BEGIN PGP PUBLIC KEY BLOCK\s*-----\s*$"#).unwrap()
    })
}

#[derive(Debug, Clone)]
pub struct GnupgClient {
    command: String,
}

impl GnupgClient {
    /// Create a new GnuPG client from the name/path of the GnuPG binary.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
        }
    }

    /// Create a new GnuPG command.
    pub(super) fn command(&self) -> Command {
        Command::new(&self.command)
    }

    /// Handle errors running a GnuPG command.
    pub(super) fn map_err(&self, err: io::Error) -> eyre::Report {
        if err.kind() == io::ErrorKind::NotFound {
            return eyre!(Error::GnupgNotFound {
                path: self.command.clone()
            });
        }

        eyre!(err)
    }

    /// Return whether this is a valid PGP key.
    fn is_pgp_key(&self, mut key: impl Read) -> eyre::Result<bool> {
        let mut process = self
            .command()
            .arg("--show-keys")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| self.map_err(err))?;

        write_stdin(&mut process, &mut key)?;

        let status = process.wait()?;

        Ok(status.success())
    }

    /// Return whether the key in `file` is armored.
    ///
    /// This probes the key's contents to determine if it's armored.
    fn probe_key_encoding(&self, key: impl Read) -> eyre::Result<KeyEncoding> {
        let mut first_line = String::new();
        let mut reader = BufReader::new(key);

        match reader.read_line(&mut first_line) {
            Ok(_) => {
                if pgp_armor_regex().is_match(&first_line) {
                    Ok(KeyEncoding::Armored)
                } else {
                    Ok(KeyEncoding::Binary)
                }
            }
            // The file is not valid UTF-8, meaning it can't be armored.
            Err(err) if err.kind() == io::ErrorKind::InvalidData => Ok(KeyEncoding::Binary),
            Err(err) => bail!(err),
        }
    }

    /// Get a key from a file path.
    pub fn read_key(&self, path: &Path) -> eyre::Result<Key> {
        let mut file = File::open(path).wrap_err("failed opening local key file for reading")?;

        file.seek(SeekFrom::Start(0))?;

        if !self.is_pgp_key(&file)? {
            bail!(Error::NotPgpKey {
                key: path.to_string_lossy().to_string(),
            });
        }

        file.seek(SeekFrom::Start(0))?;

        let mut key = Vec::new();

        file.read_to_end(&mut key)
            .wrap_err("failed reading key from vile")?;

        let encoding = self
            .probe_key_encoding(&mut key.as_slice())
            .wrap_err("failed probing if PGP key is armored")?;

        Ok(self.new_key(key, encoding, None))
    }

    /// Download a key from a url.
    pub fn download_key(&self, url: &Url) -> eyre::Result<Key> {
        let mut file = download_file(url).wrap_err("failed downloading PGP key")?;

        file.seek(SeekFrom::Start(0))?;

        if !self.is_pgp_key(&file)? {
            bail!(Error::NotPgpKey {
                key: url.to_string(),
            });
        }

        file.seek(SeekFrom::Start(0))?;

        let mut key = Vec::new();

        file.read_to_end(&mut key)
            .wrap_err("failed reading key from vile")?;

        let encoding = self
            .probe_key_encoding(&mut key.as_slice())
            .wrap_err("failed probing if PGP key is armored")?;

        Ok(self.new_key(key, encoding, None))
    }
}

#[cfg(test)]
mod tests {
    use xpct::{be_err, be_ok, equal, expect};

    use crate::error::Error;

    use super::*;

    #[test]
    fn fails_when_gpg_path_is_nonexistent() -> eyre::Result<()> {
        let key_file = tempfile::NamedTempFile::new()?;

        let gpg_bin_path = "/nonexistent";

        let client = GnupgClient::new(gpg_bin_path);

        expect!(client.read_key(key_file.path()))
            .to(be_err())
            .map(|err| err.downcast::<Error>())
            .to(be_ok())
            .to(equal(Error::GnupgNotFound {
                path: gpg_bin_path.into(),
            }));

        Ok(())
    }
}
