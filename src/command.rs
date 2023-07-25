use std::fmt::Write;

use crate::cli;
use crate::convert::EntryConverter;
use crate::entry::{OverwriteAction, SourceEntry};
use crate::key::KeyDestination;

/// A CLI command.
pub trait Command {
    fn run(&mut self) -> eyre::Result<()>;

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
            writeln!(&mut output, "Installed signing key to {}", path.display())?;
        }

        match self.action {
            crate::entry::OverwriteAction::Overwrite => writeln!(
                &mut output,
                "Overwrote source file {}",
                self.entry.path().display()
            )?,
            crate::entry::OverwriteAction::Append => writeln!(
                &mut output,
                "Appended new entry to source file {}",
                self.entry.path().display()
            )?,
            crate::entry::OverwriteAction::Fail => writeln!(
                &mut output,
                "Created new source file {}",
                self.entry.path().display()
            )?,
        };

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
            entry: SourceEntry::from_add_args(args)?,
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
            writeln!(&mut output, "Installed signing key to {}", path.display())?;
        }

        match self.action {
            crate::entry::OverwriteAction::Overwrite => writeln!(
                &mut output,
                "Overwrote source file {}",
                self.entry.path().display()
            )?,
            crate::entry::OverwriteAction::Append => writeln!(
                &mut output,
                "Appended new entry to source file {}",
                self.entry.path().display()
            )?,
            crate::entry::OverwriteAction::Fail => writeln!(
                &mut output,
                "Created new source file {}",
                self.entry.path().display()
            )?,
        };

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

        if self.converter.dest_path().is_none() {
            // We're writing the converted source entry to stdout, so we don't want to print any
            // output.
            return Ok(None);
        }

        if let Some(path) = self.converter.backup_path() {
            writeln!(
                &mut output,
                "Backed up original source file to {}",
                path.display()
            )?;
        }

        if let Some(path) = self.converter.dest_path() {
            writeln!(&mut output, "Created new source file {}", path.display())?;
        }

        if let Some(path) = self.converter.src_path() {
            writeln!(&mut output, "Removed source file {}", path.display())?;
        }

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
