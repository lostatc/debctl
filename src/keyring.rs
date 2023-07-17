use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek};
use std::path::Path;
use std::process::{Command, Stdio};

use eyre::{bail, WrapErr};
use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::Error;

/// A regex which matches the first line of an ASCII-armored public PGP key.
static PGP_ARMOR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\s*-----\s*BEGIN PGP PUBLIC KEY BLOCK\s*-----\s*$"#).unwrap());

/// A location to acquire a public singing key from.
#[derive(Debug)]
pub enum KeyLocation {
    /// Download the key from a URL.
    Download { url: String },

    /// Fetch the key from a keyserver.
    Keyserver {
        fingerprint: String,
        keyserver: String,
    },
}

/// Download a file from `url` and return its file handle.
fn download_file(url: &str) -> eyre::Result<File> {
    let mut temp_file = tempfile::tempfile()?;

    reqwest::blocking::get(url)?.copy_to(&mut temp_file)?;

    Ok(temp_file)
}

/// Return whether the key in `file` is armored.
///
/// This probes the key's contents to determine if it's armored.
fn probe_is_key_armored(file: &mut impl Read) -> eyre::Result<bool> {
    let mut first_line = String::new();
    let mut reader = BufReader::new(file);

    reader.read_line(&mut first_line)?;

    Ok(PGP_ARMOR_REGEX.is_match(&first_line))
}

/// Dearmor the key in `file` and return a new temporary file.
fn dearmor_key(file: &mut File) -> eyre::Result<File> {
    let mut process = Command::new("gpg")
        .arg("--dearmor")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdout = process.stdout.take().unwrap();
    let mut stdin = process.stdin.take().unwrap();

    let handle = std::thread::spawn(move || -> eyre::Result<File> {
        let mut dearmored_file = tempfile::tempfile()?;

        io::copy(&mut stdout, &mut dearmored_file)?;

        Ok(dearmored_file)
    });

    io::copy(file, &mut stdin)?;

    process.wait()?;

    let dearmored_file = handle
        .join()
        .expect("thread panicked writing stdout to file")?;

    Ok(dearmored_file)
}

/// Download a singing key from a keyserver to `key_path`.
fn fetch_key_from_keyserver(
    key_path: &Path,
    fingerprint: &str,
    keyserver: &str,
) -> eyre::Result<()> {
    Command::new("gpg")
        .arg("--no-default-keyring")
        .arg("--keyring")
        .arg(key_path.as_os_str())
        .arg("--keyserver")
        .arg(keyserver)
        .arg("--recv-keys")
        .arg(fingerprint)
        .spawn()?
        .wait()?;

    Ok(())
}

/// Download the key at `url` to `path`.
///
/// The file at `path` is created or truncated. If this key is armored, this dearmors it.
fn download_key(url: &str, path: &Path) -> eyre::Result<()> {
    let mut key_file = download_file(url).wrap_err("failed downloading signing key")?;

    key_file.seek(io::SeekFrom::Start(0))?;

    let key_is_armored =
        probe_is_key_armored(&mut key_file).wrap_err("failed probing if key is armored")?;

    let mut dearmored_key = if key_is_armored {
        key_file.seek(io::SeekFrom::Start(0))?;
        dearmor_key(&mut key_file).wrap_err("failed dearmoring key")?
    } else {
        key_file
    };

    dearmored_key.seek(io::SeekFrom::Start(0))?;

    let mut dest_file = match File::create(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => bail!(Error::PermissionDenied),
        Err(err) => bail!(err),
    };

    io::copy(&mut dearmored_key, &mut dest_file).wrap_err("failed copying key to destination")?;

    Ok(())
}

impl KeyLocation {
    /// Download and install the signing key to `path`.
    pub fn install(&self, path: &Path) -> eyre::Result<()> {
        match &self {
            Self::Download { url } => {
                download_key(url, path).wrap_err("failed downloading signing key")
            }
            Self::Keyserver {
                fingerprint,
                keyserver,
            } => fetch_key_from_keyserver(path, fingerprint, keyserver)
                .wrap_err("failed fetching signing key from keyserver"),
        }
    }
}
