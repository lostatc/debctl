use crate::cli::{Add, Commands, New};
use crate::source::RepoSource;

fn new(args: New) -> eyre::Result<()> {
    let mut source = RepoSource::from_new_args(args)?;

    source.install_key()?;
    source.install()?;

    Ok(())
}

fn add(args: Add) -> eyre::Result<()> {
    let mut source = RepoSource::from_add_args(args)?;

    source.install_key()?;
    source.install()?;

    Ok(())
}

impl Commands {
    pub fn dispatch(self) -> eyre::Result<()> {
        match self {
            Commands::New(args) => new(args),
            Commands::Add(args) => add(args),
        }
    }
}
