#![forbid(unsafe_code)]

mod cli;

use clap::Parser;

use cli::Cli;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let _cli = Cli::parse();

    Ok(())
}
