use std::fmt::Write;

use crate::cli;
use crate::convert::EntryConverter;
use crate::entry::{OverwriteAction, SourceEntry};
use crate::key::KeyDestination;

/// A CLI command.
pub trait Command {
    /// Run the command.
    fn run(&mut self) -> eyre::Result<()>;

    /// Return an optional report of what the command did, to print to stdout.
    fn report(&self) -> eyre::Result<Option<String>>;
}

pub struct NewCommand {
    action: OverwriteAction,
    key_dest: KeyDestination,
    entry: SourceEntry,
}

impl NewCommand {
    pub fn new(args: cli::New) -> eyre::Result<Self> {
        Ok(Self {
            action: args.overwrite.action(),
            key_dest: KeyDestination::from_args(&args.key.destination, &args.name),
            entry: SourceEntry::from_new_args(&args)?,
        })
    }
}

impl Command for NewCommand {
    fn run(&mut self) -> eyre::Result<()> {
        self.entry.install_key(&self.key_dest)?;
        self.entry.install(self.action)?;

        Ok(())
    }

    fn report(&self) -> eyre::Result<Option<String>> {
        let mut output = String::new();

        if let KeyDestination::File { path } = &self.key_dest {
            writeln!(&mut output, "Installed signing key: {}", path.display())?;
        }

        write!(&mut output, "{}", self.entry.plan(self.action)?)?;

        Ok(Some(output))
    }
}

pub struct AddCommand {
    action: OverwriteAction,
    key_dest: KeyDestination,
    entry: SourceEntry,
}

impl AddCommand {
    pub fn new(args: cli::Add) -> eyre::Result<Self> {
        Ok(Self {
            action: args.overwrite.action(),
            key_dest: KeyDestination::from_args(&args.key.destination, &args.name),
            entry: SourceEntry::from_add_args(&args)?,
        })
    }
}

impl Command for AddCommand {
    fn run(&mut self) -> eyre::Result<()> {
        self.entry.install_key(&self.key_dest)?;
        self.entry.install(self.action)?;

        Ok(())
    }

    fn report(&self) -> eyre::Result<Option<String>> {
        let mut output = String::new();

        if let KeyDestination::File { path } = &self.key_dest {
            writeln!(&mut output, "Installed signing key: {}", path.display())?;
        }

        write!(&mut output, "{}", self.entry.plan(self.action)?)?;

        Ok(Some(output))
    }
}

pub struct ConvertCommand {
    converter: EntryConverter,
}

impl ConvertCommand {
    pub fn new(args: cli::Convert) -> eyre::Result<Self> {
        Ok(Self {
            converter: EntryConverter::from_args(&args)?,
        })
    }
}

impl Command for ConvertCommand {
    fn run(&mut self) -> eyre::Result<()> {
        self.converter.convert()?;

        Ok(())
    }

    fn report(&self) -> eyre::Result<Option<String>> {
        let mut output = String::new();

        write!(&mut output, "{}", self.converter.plan())?;

        Ok(Some(output))
    }
}

impl cli::Commands {
    pub fn dispatch(self) -> eyre::Result<Box<dyn Command>> {
        match self {
            cli::Commands::New(args) => Ok(Box::new(NewCommand::new(args)?)),
            cli::Commands::Add(args) => Ok(Box::new(AddCommand::new(args)?)),
            cli::Commands::Convert(args) => Ok(Box::new(ConvertCommand::new(args)?)),
        }
    }
}
