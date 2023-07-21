use std::io;
use std::process::Command;

use eyre::eyre;

use crate::error::Error;

/// Create a GnuPG command.
pub fn gpg_command() -> Command {
    Command::new("gpg")
}

/// Handle errors running a GnuPG command.
pub fn map_gpg_err(err: io::Error) -> eyre::Report {
    if err.kind() == io::ErrorKind::NotFound {
        return eyre!(Error::GnupgNotFound);
    }

    eyre!(err)
}
