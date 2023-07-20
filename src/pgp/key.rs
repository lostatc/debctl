use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Command, Stdio};

use eyre::{bail, WrapErr};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Url;

use crate::net::download_file;

use super::keyring::Keyring;

/// A regex which matches the first line of an ASCII-armored public PGP key.
static PGP_ARMOR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\s*-----\s*BEGIN PGP PUBLIC KEY BLOCK\s*-----\s*$"#).unwrap());

/// Return whether the key in `file` is armored.
///
/// This probes the key's contents to determine if it's armored.
fn probe_key_encoding(file: &mut impl Read) -> eyre::Result<KeyEncoding> {
    let mut first_line = String::new();
    let mut reader = BufReader::new(file);

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

/// The machine-readable output of a GnuPG command.
#[derive(Debug)]
struct ColonOutput {
    lines: Vec<Vec<String>>,
}

impl ColonOutput {
    const RECORD_TYPE_INDEX: usize = 0;
    const KEY_ID_INDEX: usize = 4;

    /// Create a new instance from a gpg command's stdout.
    pub fn new(output: &str) -> Self {
        let mut lines = Vec::new();

        for line in output.lines() {
            lines.push(line.split(':').map(ToString::to_string).collect::<Vec<_>>());
        }

        Self { lines }
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
    file: File,
    encoding: KeyEncoding,
    id: Option<KeyId>,
}

impl Key {
    pub(crate) fn new(file: File, encoding: KeyEncoding, id: Option<KeyId>) -> Self {
        Self { file, encoding, id }
    }

    /// Get a key from a file path.
    pub fn from_file(path: &Path) -> eyre::Result<Self> {
        let mut file = File::open(path).wrap_err("failed opening local key file for reading")?;

        file.seek(SeekFrom::Start(0))?;

        let encoding =
            probe_key_encoding(&mut file).wrap_err("failed probing if PGP key is armored")?;

        Ok(Self {
            file,
            encoding,
            id: None,
        })
    }

    /// Download a key from `url`.
    pub fn from_url(url: &Url) -> eyre::Result<Self> {
        let mut file = download_file(url).wrap_err("failed downloading PGP key")?;

        file.seek(SeekFrom::Start(0))?;

        let encoding =
            probe_key_encoding(&mut file).wrap_err("failed probing if PGP key is armored")?;

        Ok(Self {
            file,
            encoding,
            id: None,
        })
    }

    /// The encoding of this key.
    pub fn encoding(&self) -> KeyEncoding {
        self.encoding
    }

    /// Dearmor this key.
    pub fn dearmor(mut self) -> eyre::Result<Self> {
        if self.encoding == KeyEncoding::Binary {
            return Ok(self);
        }

        let mut process = Command::new("gpg")
            .arg("--dearmor")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .wrap_err("failed to execute gpg command")?;

        let mut stdout = process.stdout.take().unwrap();
        let mut stdin = process.stdin.take().unwrap();

        let handle = std::thread::spawn(move || -> eyre::Result<File> {
            let mut dearmored_file = tempfile::tempfile()?;

            io::copy(&mut stdout, &mut dearmored_file)?;

            Ok(dearmored_file)
        });

        self.file.seek(SeekFrom::Start(0))?;

        io::copy(&mut self.file, &mut stdin)?;

        drop(stdin);

        process.wait()?;

        let dearmored_file = handle
            .join()
            .expect("thread panicked writing stdout to file")?;

        Ok(Self {
            file: dearmored_file,
            encoding: KeyEncoding::Binary,
            id: self.id,
        })
    }

    /// Enarmor this key.
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

        let mut process = Command::new("gpg")
            .arg("--show-keys")
            .arg("--with-colons")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .wrap_err("failed to execute gpg command")?;

        let mut stdout = process.stdout.take().unwrap();
        let mut stdin = process.stdin.take().unwrap();

        let handle = std::thread::spawn(move || -> eyre::Result<String> {
            let mut stdout_buf = String::new();

            stdout
                .read_to_string(&mut stdout_buf)
                .wrap_err("error reading command stdout")?;

            Ok(stdout_buf)
        });

        self.file.seek(SeekFrom::Start(0))?;

        io::copy(&mut self.file, &mut stdin)?;

        drop(stdin);

        process.wait()?;

        let command_output = handle
            .join()
            .expect("thread panicked writing stdout to file")?;

        let key_id = ColonOutput::new(&command_output)
            .public_key_id()
            .wrap_err("failed parsing gpg colon output")?;

        self.id = Some(key_id.clone());

        Ok(key_id)
    }
}

impl Read for Key {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
}

impl Seek for Key {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.file.seek(pos)
    }
}
