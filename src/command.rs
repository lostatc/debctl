use crate::cli::{Add, AddCommands, AddLine, AddNew, Commands};
use crate::source::RepoSource;

fn new(args: AddNew) -> eyre::Result<()> {
    let mut source = RepoSource::from_new_args(args)?;

    source.install_key()?;
    source.install()?;

    Ok(())
}

fn add(args: AddLine) -> eyre::Result<()> {
    let mut source = RepoSource::from_add_args(args)?;

    source.install_key()?;
    source.install()?;

    Ok(())
}

impl Commands {
    pub fn dispatch(self) -> eyre::Result<()> {
        match self {
            Commands::Add(Add { command }) => match command {
                AddCommands::New(args) => new(args),
                AddCommands::Line(args) => add(args),
                AddCommands::Ppa(_) => todo!(),
            },
        }
    }
}
