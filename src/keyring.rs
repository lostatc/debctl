use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use eyre::{bail, WrapErr};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Url;

use crate::error::Error;

/// A regex which matches the first line of an ASCII-armored public PGP key.
static PGP_ARMOR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\s*-----\s*BEGIN PGP PUBLIC KEY BLOCK\s*-----\s*$"#).unwrap());

/// A location to acquire a public singing key from.
#[derive(Debug)]
pub enum KeyLocation {
    /// Download the key from a URL.
    Download { url: Url },

    /// Copy the file from a path.
    File { path: PathBuf },

    /// Fetch the key from a keyserver.
    Keyserver {
        fingerprint: String,
        keyserver: String,
    },
}

/// Download a file from `url` and return its file handle.
fn download_file(url: &str) -> eyre::Result<File> {
    let mut temp_file = tempfile::tempfile()?;

    let mut response = reqwest::blocking::get(url)?;
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

    drop(stdin);

    process.wait()?;

    let dearmored_file = handle
        .join()
        .expect("thread panicked writing stdout to file")?;

    Ok(dearmored_file)
}

/// Open the destination file to install the key to.
fn open_key_destination(path: &Path) -> eyre::Result<File> {
    match File::create(path) {
        Ok(file) => Ok(file),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => bail!(Error::PermissionDenied),
        Err(err) => Err(err).wrap_err("failed opening destination file for writing")?,
    }
}

/// Dearmor the given key if it is ascii-armored.
///
/// Otherwise, return the file as-is.
fn dearmor_key_if_armored(mut key_file: File) -> eyre::Result<File> {
    key_file.seek(SeekFrom::Start(0))?;

    let key_is_armored =
        probe_is_key_armored(&mut key_file).wrap_err("failed probing if key is armored")?;

    let mut dearmored_key = if key_is_armored {
        key_file.seek(SeekFrom::Start(0))?;
        dearmor_key(&mut key_file).wrap_err("failed dearmoring key")?
    } else {
        key_file
    };

    dearmored_key.seek(SeekFrom::Start(0))?;

    Ok(dearmored_key)
}

/// Install the key at `url` to `dest`.
///
/// The file at `dest` is created or truncated. If this key is armored, this dearmors it.
fn download_key(url: &str, dest: &Path) -> eyre::Result<()> {
    let src_file = download_file(url).wrap_err("failed downloading signing key")?;

    let mut dearmored_key = dearmor_key_if_armored(src_file)?;

    let mut dest_file = open_key_destination(dest)?;

    io::copy(&mut dearmored_key, &mut dest_file).wrap_err("failed copying key to destination")?;

    Ok(())
}

/// Install the key at `key` to `dest`.
///
/// The file at `dest` is created or truncated. If this key is armored, this dearmors it.
fn install_local_key(key: &Path, dest: &Path) -> eyre::Result<()> {
    let src_file = File::open(key).wrap_err("failed opening local key file for reading")?;

    let mut dearmored_key = dearmor_key_if_armored(src_file)?;

    let mut dest_file = open_key_destination(dest)?;

    io::copy(&mut dearmored_key, &mut dest_file).wrap_err("failed copying key to destination")?;

    Ok(())
}

/// Download a singing key from a keyserver to `key_path`.
fn fetch_key_from_keyserver(fingerprint: &str, keyserver: &str, dest: &Path) -> eyre::Result<()> {
    Command::new("gpg")
        .arg("--no-default-keyring")
        .arg("--keyring")
        .arg(dest.as_os_str())
        .arg("--keyserver")
        .arg(keyserver)
        .arg("--recv-keys")
        .arg(fingerprint)
        .spawn()?
        .wait()?;

    Ok(())
}

impl KeyLocation {
    /// Download and install the signing key to `path`.
    pub fn install(&self, dest: &Path) -> eyre::Result<()> {
        match &self {
            Self::Download { url } => {
                download_key(url.as_str(), dest).wrap_err("failed downloading signing key")
            }
            Self::File { path: src } => {
                install_local_key(src, dest).wrap_err("failed installing key from local path")
            }
            Self::Keyserver {
                fingerprint,
                keyserver,
            } => fetch_key_from_keyserver(fingerprint, keyserver, dest)
                .wrap_err("failed fetching signing key from keyserver"),
        }
    }
}
