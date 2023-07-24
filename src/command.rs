use crate::cli::{Add, Commands, Convert, New};
use crate::convert::EntryConverter;
use crate::entry::{OverwriteAction, SourceEntry};
use crate::key::KeyDestination;
use crate::stdio::path_is_stdio;

fn print_create_action(action: OverwriteAction, key_dest: &KeyDestination, entry: &SourceEntry) {
    if let KeyDestination::File { path } = key_dest {
        println!("Installed signing key to {}", path.display());
    }

    match action {
        crate::entry::OverwriteAction::Overwrite => {
            println!("Overwrote source file {}", entry.path().display())
        }
        crate::entry::OverwriteAction::Append => {
            println!(
                "Appended new entry to source file {}",
                entry.path().display()
            )
        }
        crate::entry::OverwriteAction::Fail => {
            println!("Created new source file {}", entry.path().display())
        }
    }
}

fn new(args: New) -> eyre::Result<()> {
    let action = args.overwrite.action();
    let key_dest = KeyDestination::from_args(&args.key.destination, &args.name);
    let mut entry = SourceEntry::from_new_args(args)?;

    entry.install_key(&key_dest)?;
    entry.install(action)?;

    print_create_action(action, &key_dest, &entry);

    Ok(())
}

fn add(args: Add) -> eyre::Result<()> {
    let action = args.overwrite.action();
    let key_dest = KeyDestination::from_args(&args.key.destination, &args.name);
    let mut entry = SourceEntry::from_add_args(args)?;

    entry.install_key(&key_dest)?;
    entry.install(action)?;

    print_create_action(action, &key_dest, &entry);

    Ok(())
}

fn print_convert_action(converter: &EntryConverter) {
    if path_is_stdio(converter.dest_path().as_ref()) {
        // We're writing the file contents to stdout.
        return;
    }

    if let Some(path) = converter.backup_path() {
        println!("Backed up original source file to {}", path.display());
    }

    println!(
        "Created new source file {}",
        converter.dest_path().display()
    );

    if !path_is_stdio(converter.src_path().as_ref()) {
        println!("Removed source file {}", converter.src_path().display());
    }
}

fn convert(args: Convert) -> eyre::Result<()> {
    let converter = EntryConverter::from_args(&args)?;

    converter.convert()?;

    print_convert_action(&converter);

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
