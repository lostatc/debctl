use std::path::PathBuf;
use std::process::Command;

use eyre::WrapErr;

use crate::cli::AddNew;
use crate::cli::SourceType;
use crate::keyring::get_keyring_path;

/// A Debian repository source.
#[derive(Debug)]
pub struct RepoSource {
    name: String,
    uri: String,
    description: Option<String>,
    suites: Vec<String>,
    components: Vec<String>,
    kind: Vec<SourceType>,
    key_path: PathBuf,
    architectures: Vec<String>,
    languages: Vec<String>,
    targets: Vec<String>,
    pdiffs: bool,
    by_hash: bool,
    enabled: bool,
}

/// Return the current distro version codename.
fn get_current_codename() -> eyre::Result<String> {
    Ok(String::from_utf8(
        Command::new("lsb_release")
            .arg("--short")
            .arg("--codename")
            .output()
            .wrap_err("failed getting distro version codename")?
            .stdout,
    )?)
}

impl RepoSource {
    /// Construct an instance from the CLI `args`.
    ///
    /// This does not download the signing key.
    pub fn from_cli(args: AddNew) -> eyre::Result<Self> {
        Ok(Self {
            name: args.name.clone(),
            uri: args.uri,
            description: args.description,
            suites: if args.suites.is_empty() {
                vec![get_current_codename()?]
            } else {
                args.suites
            },
            components: args.components,
            kind: args.kind,
            key_path: get_keyring_path(&args.name),
            architectures: args.arch,
            languages: args.lang,
            targets: args.targets,
            pdiffs: args.pdiffs,
            by_hash: args.by_hash,
            enabled: !args.disable,
        })
    }
}
