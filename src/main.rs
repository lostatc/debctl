#![forbid(unsafe_code)]

mod cli;
mod command;
mod convert;
mod entry;
mod error;
mod file;
mod key;
mod option;
mod parse;
mod pgp;
mod stdio;
mod test;
mod types;

use std::process::ExitCode;

use clap::Parser;
use cli::Cli;
use error::Error;
use pgp::set_gpg_path;

fn run() -> eyre::Result<()> {
    let cli = Cli::parse();

    set_gpg_path(&cli.gpg_path);

    let mut command = cli.dispatch()?;

    if !cli.dry_run {
        command.run()?;
    }

    if let Some(report) = command.report()? {
        eprintln!("{}", report);
    }

    Ok(())
}

fn main() -> eyre::Result<ExitCode> {
    color_eyre::install()?;

    if let Err(err) = run() {
        // User-facing errors should not show a stack trace.
        if let Some(user_err) = err.downcast_ref::<Error>() {
            eprintln!("{}", user_err);
            return Ok(ExitCode::FAILURE);
        }

        return Err(err);
    }

    Ok(ExitCode::SUCCESS)
}
