use std::io;
use std::process::Command;
use std::sync::OnceLock;

use eyre::eyre;

use crate::error::Error;

/// The name/path of the GnuPG binary.
static GPG_PATH: OnceLock<String> = OnceLock::new();

/// Set the path of the GnuPG binary.
///
/// This panics if called more than once.
pub fn set_gpg_path(path: &str) {
    GPG_PATH
        .set(path.to_owned())
        .expect("tried to set the GnuPG path more than once");
}

/// Return the path of the GnuPG binary.
///
/// This panics if `set_gpg_path` was never called.
fn get_gpg_path() -> &'static str {
    GPG_PATH
        .get()
        .expect("the path of the GnuPG binary was never set")
}

/// Create a GnuPG command.
pub fn gpg_command() -> Command {
    Command::new(get_gpg_path())
}

/// Handle errors running a GnuPG command.
pub fn map_gpg_err(err: io::Error) -> eyre::Report {
    if err.kind() == io::ErrorKind::NotFound {
        return eyre!(Error::GnupgNotFound {
            path: get_gpg_path().to_owned()
        });
    }

    eyre!(err)
}
