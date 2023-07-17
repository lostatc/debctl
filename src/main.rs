#![forbid(unsafe_code)]
// TODO: Remove
#![allow(dead_code)]

mod cli;
mod command;
mod error;
mod format;
mod keyring;
mod option;
mod source;

use clap::Parser;

use cli::Cli;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    cli.command.dispatch()?;

    Ok(())
}
