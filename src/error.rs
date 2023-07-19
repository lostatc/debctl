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

    #[error("This source file already exists: `{path}`.\n\nYou can either overwrite it with `--overwrite` or pick a different name for the source.")]
    SourceFileAlreadyExists { path: PathBuf },

    #[error("You must run this command as root.")]
    PermissionDenied,

    #[error("This single-line-style entry is malformed.\n\n{reason}")]
    MalformedSingleLineEntry { reason: String },

    #[error("This key is not a valid URL or file path: `{path}`.")]
    InvalidKeyLocation { path: String },
}
