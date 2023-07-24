use std::io::{self, Read};
use std::path::Path;
use std::process::Child;
use std::thread::JoinHandle;

use eyre::{bail, WrapErr};

/// Write to a command's stdin and close the stream.
pub fn write_stdin(process: &mut Child, mut src: impl Read) -> eyre::Result<()> {
    let mut stdin = process.stdin.take().unwrap();

    io::copy(&mut src, &mut stdin).wrap_err("failed writing to stdin")?;

    // This implicitly drops `stdin`, closing the stream.

    Ok(())
}

/// A handle for collecting a command's stdout in a separate thread.
#[derive(Debug)]
pub struct StdoutHandle(JoinHandle<eyre::Result<Vec<u8>>>);

impl StdoutHandle {
    /// Join the thread and return the command's stdout.
    pub fn join(self) -> eyre::Result<Vec<u8>> {
        self.0
            .join()
            .expect("thread panicked reading command stdout")
    }
}

/// Start consuming a command's stdout in a new thread..
pub fn read_stdout(process: &mut Child) -> StdoutHandle {
    let mut stdout = process.stdout.take().unwrap();

    StdoutHandle(std::thread::spawn(move || {
        let mut output = Vec::new();

        io::copy(&mut stdout, &mut output).wrap_err("failed reading from stdout")?;

        Ok(output)
    }))
}

/// A handle for collecting a command's stderr in a separate thread.
#[derive(Debug)]
pub struct StderrHandle(JoinHandle<eyre::Result<String>>);

/// Start consuming a command's stderr in a new thread.
pub fn read_stderr(process: &mut Child) -> StderrHandle {
    let mut stderr = process.stderr.take().unwrap();

    StderrHandle(std::thread::spawn(move || {
        let mut output = String::new();

        stderr
            .read_to_string(&mut output)
            .wrap_err("failed reading from stderr")?;

        Ok(output)
    }))
}

/// Wait for the command to finish and return an error if it failed.
pub fn wait(mut process: Child, handle: StderrHandle) -> eyre::Result<()> {
    let status = process.wait()?;

    let err_msg = handle
        .0
        .join()
        .expect("thread panicked reading command stderr")?;

    if !status.success() {
        bail!(err_msg);
    }

    Ok(())
}

/// Return whether this path is "-", meaning to read from stdin or write to stdout.
pub fn path_is_stdio(path: &Path) -> bool {
    path == Path::new("-")
}
