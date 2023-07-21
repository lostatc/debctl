use crate::cli::{Add, Commands, Convert, New};
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

fn convert(_: Convert) -> eyre::Result<()> {
    todo!()
}

impl Commands {
    pub fn dispatch(self) -> eyre::Result<()> {
        match self {
            Commands::New(args) => new(args),
            Commands::Add(args) => add(args),
            Commands::Convert(args) => convert(args),
        }
    }
}
