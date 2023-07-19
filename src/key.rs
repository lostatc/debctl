use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use eyre::{bail, eyre, WrapErr};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Url;

use crate::error::Error;
use crate::option::OptionValue;

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

/// The encoding of a PGP key.
#[derive(Debug, Clone, Copy)]
enum KeyEncoding {
    Armored,
    Binary,
}

/// Encode the PGP key in `file` and return a new temporary file.
fn encode_key(file: &mut File, action: KeyEncoding) -> eyre::Result<File> {
    let mut process = Command::new("gpg")
        .arg(match action {
            KeyEncoding::Armored => "--armor",
            KeyEncoding::Binary => "--dearmor",
        })
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

/// Change the encoding of a PGP key.
///
/// This leaves the key as-is if it's already encoded that way.
fn change_key_encoding(mut key_file: File, action: KeyEncoding) -> eyre::Result<File> {
    key_file.seek(SeekFrom::Start(0))?;

    let key_is_armored =
        probe_is_key_armored(&mut key_file).wrap_err("failed probing if key is armored")?;

    key_file.seek(SeekFrom::Start(0))?;

    let mut encoded_key = match (key_is_armored, action) {
        (true, KeyEncoding::Binary) => {
            encode_key(&mut key_file, action).wrap_err("failed dearmoring key")?
        }
        (false, KeyEncoding::Armored) => {
            encode_key(&mut key_file, action).wrap_err("failed armoring key")?
        }
        _ => key_file,
    };

    encoded_key.seek(SeekFrom::Start(0))?;

    Ok(encoded_key)
}

fn ensure_dir_exists(keyring_dir: &Path) -> eyre::Result<()> {
    match fs::create_dir_all(keyring_dir) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => bail!(Error::PermissionDenied),
        Err(err) => Err(err).wrap_err("failed creating directory"),
    }
}

/// Download a singing key from a keyserver and return it.
fn fetch_key_from_keyserver(fingerprint: &str, keyserver: &str) -> eyre::Result<File> {
    // This file will be the temporary keyring we will store the key in when we initially fetch it
    // from the keyserver. This is important because GPG doesn't allow us to read the key directly
    // to stdout; we must store it in a keyring.
    let temp_keyring =
        tempfile::NamedTempFile::new().wrap_err("failed creating temporary keyring file")?;

    let recv_cmd_output = Command::new("gpg")
        .arg("--no-default-keyring")
        .arg("--keyring")
        .arg(temp_keyring.path().as_os_str())
        .arg("--keyserver")
        .arg(keyserver)
        .arg("--recv-keys")
        .arg(fingerprint)
        .output()
        .wrap_err("failed to execute gpg command")?;

    if !recv_cmd_output.status.success() {
        bail!(Error::KeyserverFetchFailed {
            fingerprint: fingerprint.to_string(),
            reason: String::from_utf8(recv_cmd_output.stderr)
                .wrap_err("failed to decode gpg command stderr")?,
        });
    }

    let mut export_cmd_process = Command::new("gpg")
        .arg("--no-default-keyring")
        .arg("--keyring")
        .arg(temp_keyring.path().as_os_str())
        .arg("--export")
        .arg(fingerprint)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err("failed to execute gpg command")?;

    let mut stdout = export_cmd_process.stdout.take().unwrap();
    let mut stderr = export_cmd_process.stderr.take().unwrap();

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

    let status = export_cmd_process.wait()?;

    let key_file = stdout_handle
        .join()
        .expect("thread panicked writing stdout to file")?;
    let err_msg = stderr_handle
        .join()
        .expect("thread panicked reading stderr")?;

    if !status.success() {
        return Err(eyre!(err_msg).wrap_err("failed to export key from temporary keyring"));
    }

    Ok(key_file)
}

impl KeyLocation {
    /// Get the signing key at this location.
    fn get_key(&self) -> eyre::Result<File> {
        match self {
            Self::Download { url } => {
                // download_key(url.as_str(), dest).wrap_err("failed downloading signing key")
                download_file(url.as_str()).wrap_err("failed downloading signing key")
            }
            Self::File { path } => {
                // install_local_key(src, dest).wrap_err("failed installing signing key from local path")},
                File::open(path).wrap_err("failed opening local key file for reading")
            }
            Self::Keyserver {
                fingerprint,
                keyserver,
            } => fetch_key_from_keyserver(fingerprint, keyserver)
                .wrap_err("failed fetching signing key from keyserver"),
        }
    }

    /// Install the signing key at this location to `dest`.
    pub fn install(&self, dest: &Path) -> eyre::Result<()> {
        if let Some(keyring_dir) = dest.parent() {
            ensure_dir_exists(keyring_dir).wrap_err("failed creating keyring directory")?;
        }

        let key_file = self.get_key().wrap_err("failed getting signing key")?;

        let mut dearmored_key = change_key_encoding(key_file, KeyEncoding::Binary)?;

        let mut dest_file = open_key_destination(dest)?;

        io::copy(&mut dearmored_key, &mut dest_file)
            .wrap_err("failed copying key to destination")?;

        Ok(())
    }

    /// Get the key at this location as an option value.
    pub fn to_value(&self) -> eyre::Result<OptionValue> {
        let key_file = self.get_key().wrap_err("failed getting signing key")?;

        Ok(OptionValue::Multiline(
            BufReader::new(key_file)
                .lines()
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}
