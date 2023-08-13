use std::path::Path;

use reqwest::Url;

/// The encoding of a PGP key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEncoding {
    Armored,
    Binary,
}

/// A PGP key.
#[derive(Debug, Clone)]
pub struct Key {
    bytes: Vec<u8>,
}

impl Key {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl AsRef<[u8]> for Key {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_slice()
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

pub trait PgpClient {
    /// Read a PGP key from a file.
    fn read_key(&self, path: &Path, encoding: KeyEncoding) -> eyre::Result<Key>;

    /// Download a PGP key from a URL.
    fn download_key(&self, url: &Url, encoding: KeyEncoding) -> eyre::Result<Key>;

    /// Receive a PGP key from a keyserver.
    fn recv_key(&self, keyserver: &str, id: KeyId, encoding: KeyEncoding) -> eyre::Result<Key>;
}
