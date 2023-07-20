#![forbid(unsafe_code)]
// TODO: Remove
#![allow(dead_code)]

mod cli;
mod command;
mod error;
mod gpg;
mod key;
mod net;
mod option;
mod parse;
mod source;
mod types;

use std::process::ExitCode;

use clap::Parser;
use cli::Cli;
use error::Error;

fn main() -> eyre::Result<ExitCode> {
    color_eyre::install()?;

    let cli = Cli::parse();

    if let Err(err) = cli.command.dispatch() {
        // User-facing errors should not show a stack trace.
        if let Some(user_facing_err) = err.downcast_ref::<Error>() {
            eprintln!("{}", user_facing_err);
            return Ok(ExitCode::FAILURE);
        }

        return Err(err);
    }

    Ok(ExitCode::SUCCESS)
}
