use std::path::PathBuf;

use thiserror::Error;

/// A error type for "expected" errors.
///
/// This type represents errors expected in common usage of the program that should trigger a
/// readable error message instead of a stack trace.
#[derive(Debug, Error)]
pub enum Error {
    #[error("This option name is invalid: {name}")]
    InvalidOptionName { name: String },

    #[error("This option is not in `key=value` format: {option}")]
    MalformedOption { option: String },

    #[error("This source file already exists: {path}")]
    SourceFileAlreadyExists { path: PathBuf },

    #[error("You must run this command as root")]
    PermissionDenied,
}
