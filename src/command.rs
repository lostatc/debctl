use std::fmt::Write;
use std::path::PathBuf;

use crate::args::{AddArgs, ConvertArgs, NewArgs, OverwriteAction};
use crate::cli;
use crate::convert::EntryConverter;
use crate::entry::SourceEntry;
use crate::file::{SourceFile, SourceFileKind, SourceFilePath};
use crate::key::KeyDest;
use crate::pgp::GnupgClient;

/// High-level configuration for the program.
pub struct Config {
    /// The path of the GnuPG binary.
    pub gpg_path: String,

    /// The path of the APT sources directory.
    pub sources_dir: PathBuf,
}

impl Config {
    /// Create a new PGP client.
    pub fn pgp_client(&self) -> GnupgClient {
        GnupgClient::new(&self.gpg_path)
    }
}

/// A CLI command.
pub trait Command {
    /// Run the command.
    fn run(&mut self) -> eyre::Result<()>;

    /// Return an optional report of what the command did, to print to stdout.
    fn report(&self) -> eyre::Result<Option<String>>;
}

pub struct NewCommand {
    client: GnupgClient,
    action: OverwriteAction,
    key_dest: KeyDest,
    entry: SourceEntry,
    source_file: SourceFile,
}

impl NewCommand {
    pub fn new(args: cli::New, conf: Config) -> eyre::Result<Self> {
        let new_args = NewArgs::from_cli(args)?;

        Ok(Self {
            client: conf.pgp_client(),
            action: new_args.action(),
            key_dest: new_args.key().dest.clone(),
            entry: SourceEntry::from_new(&new_args)?,
            source_file: SourceFile {
                path: SourceFilePath::Installed {
                    name: new_args.name().to_owned(),
                    dir: conf.sources_dir,
                },
                kind: SourceFileKind::Deb822,
            },
        })
    }
}

impl Command for NewCommand {
    fn run(&mut self) -> eyre::Result<()> {
        self.entry.install_key(&self.client, &self.key_dest)?;
        self.entry.install(&self.source_file, self.action)?;

        Ok(())
    }

    fn report(&self) -> eyre::Result<Option<String>> {
        let mut output = String::new();

        if let KeyDest::File { path } = &self.key_dest {
            writeln!(&mut output, "Installed signing key: {}", path.display())?;
        }

        write!(
            &mut output,
            "{}",
            self.entry.plan(&self.source_file, self.action)?
        )?;

        Ok(Some(output))
    }
}

pub struct AddCommand {
    client: GnupgClient,
    action: OverwriteAction,
    key_dest: KeyDest,
    entry: SourceEntry,
    source_file: SourceFile,
}

impl AddCommand {
    pub fn new(args: cli::Add, conf: Config) -> eyre::Result<Self> {
        let add_args = AddArgs::from_cli(args)?;

        Ok(Self {
            client: conf.pgp_client(),
            action: add_args.action(),
            key_dest: add_args.key().dest.clone(),
            entry: SourceEntry::from_add(&add_args)?,
            source_file: SourceFile {
                path: SourceFilePath::Installed {
                    name: add_args.name().to_owned(),
                    dir: conf.sources_dir,
                },
                kind: SourceFileKind::Deb822,
            },
        })
    }
}

impl Command for AddCommand {
    fn run(&mut self) -> eyre::Result<()> {
        self.entry.install_key(&self.client, &self.key_dest)?;
        self.entry.install(&self.source_file, self.action)?;

        Ok(())
    }

    fn report(&self) -> eyre::Result<Option<String>> {
        let mut output = String::new();

        if let KeyDest::File { path } = &self.key_dest {
            writeln!(&mut output, "Installed signing key: {}", path.display())?;
        }

        write!(
            &mut output,
            "{}",
            self.entry.plan(&self.source_file, self.action)?
        )?;

        Ok(Some(output))
    }
}

pub struct ConvertCommand {
    converter: EntryConverter,
}

impl ConvertCommand {
    pub fn new(args: cli::Convert, conf: Config) -> eyre::Result<Self> {
        Ok(Self {
            converter: EntryConverter::new(&ConvertArgs::from_cli(&args)?, conf.sources_dir)?,
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

impl cli::Cli {
    fn config(&self) -> Config {
        Config {
            gpg_path: self.gpg_path.clone(),
            sources_dir: self.sources_dir.clone(),
        }
    }

    pub fn dispatch(&self) -> eyre::Result<Box<dyn Command>> {
        let conf = self.config();

        match &self.command {
            cli::Commands::New(args) => Ok(Box::new(NewCommand::new(args.clone(), conf)?)),
            cli::Commands::Add(args) => Ok(Box::new(AddCommand::new(args.clone(), conf)?)),
            cli::Commands::Convert(args) => Ok(Box::new(ConvertCommand::new(args.clone(), conf)?)),
        }
    }
}
