use crate::cli::{Add, AddCommands, AddLine, AddNew, Commands};
use crate::source::{key_path, source_path, RepoSource};

fn add_new(args: AddNew) -> eyre::Result<()> {
    let source = RepoSource::from_add_new_args(args)?;

    if let Some(key_location) = &source.key {
        key_location.install(&key_path(&source.name))?;
    }

    source.install(&source_path(&source.name))?;

    Ok(())
}

fn add_line(args: AddLine) -> eyre::Result<()> {
    let source = RepoSource::from_add_line_args(args)?;

    if let Some(key_location) = &source.key {
        key_location.install(&key_path(&source.name))?;
    }

    source.install(&source_path(&source.name))?;

    Ok(())
}

impl Commands {
    pub fn dispatch(self) -> eyre::Result<()> {
        match self {
            Commands::Add(Add { command }) => match command {
                AddCommands::New(args) => add_new(args),
                AddCommands::Line(args) => add_line(args),
                AddCommands::Ppa(_) => todo!(),
            },
        }
    }
}
