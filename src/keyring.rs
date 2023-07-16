use std::path::PathBuf;

const KEYRING_DIR: &str = "/usr/share/keyrings";

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