use std::path::PathBuf;

use thiserror::Error;

/// A error type for user-facing errors.
///
/// This type represents errors expected in common usage of the program that should trigger a
/// readable error message instead of a stack trace.
#[derive(Debug, Error)]
pub enum Error {
    #[error("This is not a valid option name: `{name}`.\n\nSee the sources.list(5) man page for a list of valid options or use `--force-literal-options`.")]
    InvalidOptionName { name: String },

    #[error("This option is not in `key=value` format: `{option}`.")]
    MalformedOption { option: String },

    #[error("This source file already exists: `{path}`.\n\nYou can either:\n* Overwrite it with `--overwrite`\n* Append to it with `--append`\n* Pick a different name for the source")]
    SourceFileAlreadyExists { path: PathBuf },

    #[error("You must run this command as root.")]
    PermissionDenied,

    #[error("This single-line-style entry is malformed.\n\n{reason}")]
    MalformedSingleLineEntry { reason: String },

    #[error("This key is not a valid URL or file path: `{path}`.")]
    InvalidKeyLocation { path: String },

    #[error("Failed to download key from URL: `{url}`.\n\n{reason}")]
    KeyDownloadFailed { url: String, reason: String },

    #[error("You cannot pass the `Signed-By` option without also passing `--force-no-key`.\n\nYou should typically use `--key` to specify the signing key.")]
    ConflictingKeyLocations,

    #[error("Failed to fetch key from the keyserver: `{id}`.\n\n{reason}")]
    KeyserverFetchFailed { id: String, reason: String },

    #[error("Could not find `gpg` command on your `PATH`.\n\nIs GnuPG installed?")]
    GnupgNotFound,

    #[error("This is not a valid PGP key: `{key}`.")]
    NotPgpKey { key: String },
}
