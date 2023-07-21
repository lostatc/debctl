use std::io::{self, Read};
use std::process::{Child, Command};
use std::thread::JoinHandle;

use eyre::{bail, WrapErr};

/// Run a GnuPG command.
pub fn gpg_command() -> Command {
    Command::new("gpg")
}

/// Write to a command's stdin and close the stream.
pub fn write_stdin(process: &mut Child, src: &mut impl Read) -> eyre::Result<()> {
    let mut stdin = process.stdin.take().unwrap();

    io::copy(src, &mut stdin).wrap_err("failed writing to stdin")?;

    // This implicitly drops `stdin`, closing the stream.

    Ok(())
}

type StdoutHandle = JoinHandle<eyre::Result<Vec<u8>>>;

/// Start consuming a command's stdout in a new thread..
pub fn read_stdout(process: &mut Child) -> StdoutHandle {
    let mut stdout = process.stdout.take().unwrap();

    std::thread::spawn(move || {
        let mut output = Vec::new();

        io::copy(&mut stdout, &mut output).wrap_err("failed reading from stdout")?;

        Ok(output)
    })
}

type StderrHandle = JoinHandle<eyre::Result<String>>;

/// Start consuming a command's stderr in a new thread.
pub fn read_stderr(process: &mut Child) -> StderrHandle {
    let mut stderr = process.stderr.take().unwrap();

    std::thread::spawn(move || {
        let mut output = String::new();

        stderr
            .read_to_string(&mut output)
            .wrap_err("failed reading from stderr")?;

        Ok(output)
    })
}

/// Wait for the command to finish and return an error if it failed.
pub fn wait(mut process: Child, handle: StderrHandle) -> eyre::Result<()> {
    let status = process.wait()?;

    let err_msg = handle.join().unwrap()?;

    if !status.success() {
        bail!(err_msg);
    }

    Ok(())
}
