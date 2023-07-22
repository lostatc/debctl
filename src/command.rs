use crate::cli::{Add, Commands, Convert, New};
use crate::source::{KeyDestination, SourceEntry};

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
