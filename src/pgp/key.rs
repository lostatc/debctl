use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::process::Stdio;

use eyre::{bail, WrapErr};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Url;

use crate::error::Error;
use crate::stdio::{read_stderr, read_stdout, wait, write_stdin};

use super::command::{gpg_command, map_gpg_err};
use super::keyring::Keyring;

/// A regex which matches the first line of an ASCII-armored public PGP key.
static PGP_ARMOR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\s*-----\s*BEGIN PGP PUBLIC KEY BLOCK\s*-----\s*$"#).unwrap());

/// Return whether the key in `file` is armored.
///
/// This probes the key's contents to determine if it's armored.
fn probe_key_encoding(key: impl Read) -> eyre::Result<KeyEncoding> {
    let mut first_line = String::new();
    let mut reader = BufReader::new(key);

    match reader.read_line(&mut first_line) {
        Ok(_) => {
            if PGP_ARMOR_REGEX.is_match(&first_line) {
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

/// Return whether this is a valid PGP key.
fn is_pgp_key(mut key: impl Read) -> eyre::Result<bool> {
    let mut process = gpg_command()
        .arg("--show-keys")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(map_gpg_err)?;

    write_stdin(&mut process, &mut key)?;

    let status = process.wait()?;

    Ok(status.success())
}

/// Download a file from `url` and return its file handle.
fn download_file(url: &Url) -> eyre::Result<File> {
    let mut temp_file = tempfile::tempfile()?;

    let mut response = reqwest::blocking::get(url.clone())?;
    let status = response.status();

    if status.is_success() {
        response.copy_to(&mut temp_file)?;
    } else {
        bail!(Error::KeyDownloadFailed {
            url: url.to_string(),
            reason: match status.canonical_reason() {
                Some(reason_phrase) => format!("Error: {}", reason_phrase),
                None => format!("Error Code: {}", status.as_str()),
            }
        })
    }

    Ok(temp_file)
}

/// The machine-readable output of a GnuPG command.
#[derive(Debug)]
struct ColonOutput {
    lines: Vec<Vec<String>>,
}

impl ColonOutput {
    const RECORD_TYPE_INDEX: usize = 0;
    const KEY_ID_INDEX: usize = 4;

    /// Create a new instance from a gpg command's stdout.
    pub fn new(output: &[u8]) -> eyre::Result<Self> {
        let mut lines = Vec::new();

        for line_result in output.lines() {
            let line = line_result.wrap_err("error decoding command output")?;

            lines.push(line.split(':').map(ToString::to_string).collect::<Vec<_>>());
        }

        Ok(Self { lines })
    }

    /// Get the key ID of the public key.
    pub fn public_key_id(&self) -> eyre::Result<KeyId> {
        for line in &self.lines {
            let record_type = match line.get(Self::RECORD_TYPE_INDEX) {
                Some(record_type) => record_type,
                None => bail!("could not find record type in gpg colon output"),
            };

            if record_type != "pub" {
                continue;
            }

            match line.get(Self::KEY_ID_INDEX) {
                Some(key_id) => return Ok(KeyId(key_id.to_string())),
                None => bail!("could not find key id in gpg colon output"),
            }
        }

        bail!("could not find public key record in gpg colon output");
    }
}

#[derive(Debug, Clone)]
pub struct KeyId(String);

impl KeyId {
    pub fn new(id: String) -> Self {
        Self(id)
    }
}

impl AsRef<str> for KeyId {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

/// The encoding of a PGP key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEncoding {
    Armored,
    Binary,
}

/// A PGP key in a file.
#[derive(Debug)]
pub struct Key {
    key: Vec<u8>,
    encoding: KeyEncoding,
    id: Option<KeyId>,
}

impl Key {
    pub(crate) fn new(key: Vec<u8>, encoding: KeyEncoding, id: Option<KeyId>) -> Self {
        Self { key, encoding, id }
    }

    /// Get a key from a file path.
    pub fn from_file(path: &Path) -> eyre::Result<Self> {
        let mut file = File::open(path).wrap_err("failed opening local key file for reading")?;

        file.seek(SeekFrom::Start(0))?;

        if !is_pgp_key(&file)? {
            bail!(Error::NotPgpKey {
                key: path.to_string_lossy().to_string(),
            });
        }

        file.seek(SeekFrom::Start(0))?;

        let mut key = Vec::new();

        file.read_to_end(&mut key)
            .wrap_err("failed reading key from vile")?;

        let encoding = probe_key_encoding(&mut key.as_slice())
            .wrap_err("failed probing if PGP key is armored")?;

        Ok(Self {
            key,
            encoding,
            id: None,
        })
    }

    /// Download a key from `url`.
    pub fn from_url(url: &Url) -> eyre::Result<Self> {
        let mut file = download_file(url).wrap_err("failed downloading PGP key")?;

        file.seek(SeekFrom::Start(0))?;

        if !is_pgp_key(&file)? {
            bail!(Error::NotPgpKey {
                key: url.to_string(),
            });
        }

        file.seek(SeekFrom::Start(0))?;

        let mut key = Vec::new();

        file.read_to_end(&mut key)
            .wrap_err("failed reading key from vile")?;

        let encoding = probe_key_encoding(&mut key.as_slice())
            .wrap_err("failed probing if PGP key is armored")?;

        Ok(Self {
            key,
            encoding,
            id: None,
        })
    }

    /// The encoding of this key.
    pub fn encoding(&self) -> KeyEncoding {
        self.encoding
    }

    /// Dearmor this key.
    pub fn dearmor(self) -> eyre::Result<Self> {
        if self.encoding == KeyEncoding::Binary {
            return Ok(self);
        }

        let mut process = gpg_command()
            .arg("--dearmor")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(map_gpg_err)?;

        let stdout_handle = read_stdout(&mut process);
        let stderr_handle = read_stderr(&mut process);

        write_stdin(&mut process, &mut self.key.as_slice())?;

        wait(process, stderr_handle)?;

        let dearmored_key = stdout_handle.join()?;

        Ok(Self {
            key: dearmored_key,
            encoding: KeyEncoding::Binary,
            id: self.id,
        })
    }

    /// Armor this key.
    pub fn enarmor(mut self) -> eyre::Result<Self> {
        if self.encoding == KeyEncoding::Armored {
            return Ok(self);
        }

        let mut keyring = Keyring::new().wrap_err("failed creating keyring")?;

        let keyring_key = keyring
            .import(&mut self)
            .wrap_err("failed importing key into keyring")?;

        keyring
            .export(keyring_key, KeyEncoding::Armored)
            .wrap_err("failed exporting key from keyring")
    }

    /// Return the key's key ID.
    pub fn id(&mut self) -> eyre::Result<KeyId> {
        if let Some(id) = &self.id {
            return Ok(id.clone());
        }

        let mut process = gpg_command()
            .arg("--show-keys")
            .arg("--with-colons")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(map_gpg_err)?;

        let stdout_handle = read_stdout(&mut process);
        let stderr_handle = read_stderr(&mut process);

        write_stdin(&mut process, &mut self.key.as_slice())?;

        wait(process, stderr_handle)?;

        let command_output = stdout_handle.join()?;

        let key_id = ColonOutput::new(&command_output)?
            .public_key_id()
            .wrap_err("failed parsing gpg output")?;

        self.id = Some(key_id.clone());

        Ok(key_id)
    }
}

impl AsRef<[u8]> for Key {
    fn as_ref(&self) -> &[u8] {
        self.key.as_slice()
    }
}
