#![forbid(unsafe_code)]
// TODO: Remove
#![allow(dead_code)]

mod cli;
mod format;
mod keyring;

use clap::Parser;

use cli::Cli;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let _cli = Cli::parse();

    Ok(())
}
