use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Stdio, Command};

use eyre::{eyre, WrapErr};

const KEYRING_DIR: &str = "/usr/share/keyrings";
const PGP_ARMOR_HEADER: &[u8] = b"-----BEGIN PGP PUBLIC KEY BLOCK-----";

/// Return the target path of a signing key for a source.
pub fn get_keyring_path(source_name: &str) -> PathBuf {
    [KEYRING_DIR, &format!("{source_name}-archive-keyring.gpg")]
        .iter()
        .collect()
}

/// A source to acquire a public singing key from.
#[derive(Debug)]
pub enum RepoKeySource {
    /// Download the key from a URL.
    Download {
        url: String,
    },

    /// Fetch the key from a keyserver.
    Keyserver {
        fingerprint: String,
        keyserver: String,
    },
}

fn download_file(url: &str) -> eyre::Result<File> {
    let mut temp_file = tempfile::tempfile()?;

    reqwest::blocking::get(url)?.copy_to(&mut temp_file)?;

    Ok(temp_file)
}

fn probe_is_key_armored(file: &mut impl Read) -> eyre::Result<bool> {
    let mut buf = vec![0u8; PGP_ARMOR_HEADER.len()];

    match file.read_exact(&mut buf) {
        Ok(_) => Ok(buf == PGP_ARMOR_HEADER),
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(err) => Err(eyre!(err)),
    }
}

fn dearmor_key(file: &mut File) -> eyre::Result<File> {
    let mut process = Command::new("gpg")
        .arg("--dearmor")
        .stdin(Stdio::piped())
        .spawn()?;

    io::copy(file, process.stdin.as_mut().unwrap())?;

    process.wait()?;

    let mut dearmored_file = tempfile::tempfile()?;

    io::copy(process.stdout.as_mut().unwrap(), &mut dearmored_file)?;

    Ok(dearmored_file)
}

fn fetch_key_from_keyserver(key_path: &Path, fingerprint: &str, keyserver: &str) -> eyre::Result<()> {
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

impl RepoKeySource {
    /// Install the signing key at the given path.
    pub fn install(&self, path: &Path) -> eyre::Result<()> {
        match &self {
            Self::Download { url } => {
                let mut key_file = download_file(url).wrap_err("failed downloading signing key")?;

                let mut dearmored_key = if probe_is_key_armored(&mut key_file).wrap_err("failed probing if key is armored")? {
                    dearmor_key(&mut key_file).wrap_err("failed dearmoring key")?
                } else {
                    key_file
                };

                let mut dest_file = File::create(path)?;

                io::copy(&mut dearmored_key, &mut dest_file).wrap_err("failed copying key to destination")?;
            },
            Self::Keyserver { fingerprint, keyserver } => {
                fetch_key_from_keyserver(path, fingerprint, keyserver).wrap_err("failed fetching key from keyserver")?;
            },
        }

        Ok(())
    }
}