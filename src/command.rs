use crate::cli::{Add, Commands, Convert, New};
use crate::convert::EntryConverter;
use crate::entry::SourceEntry;
use crate::key::KeyDestination;

fn new(args: New) -> eyre::Result<()> {
    let action = args.overwrite.action();
    let key_dest = KeyDestination::from_args(&args.key.destination, &args.name);
    let mut source = SourceEntry::from_new_args(args)?;

    source.install_key(key_dest)?;
    source.install(action)?;

    Ok(())
}

fn add(args: Add) -> eyre::Result<()> {
    let action = args.overwrite.action();
    let key_dest = KeyDestination::from_args(&args.key.destination, &args.name);
    let mut source = SourceEntry::from_add_args(args)?;

    source.install_key(key_dest)?;
    source.install(action)?;

    Ok(())
}

fn convert(args: Convert) -> eyre::Result<()> {
    let converter = EntryConverter::from_args(&args)?;

    converter.convert()?;

    Ok(())
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
