use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::process::Command;

use eyre::{bail, eyre, WrapErr};

use crate::error::Error;

const LSB_RELEASE_CMD: &str = "lsb_release";
const OS_RELEASE_PATH: &str = "/etc/os-release";
const VERSION_CODENAME_KEY: &str = "VERSION_CODENAME";

/// Return the current distro version codename using the `lsb_release` command.
fn from_lsb_release() -> eyre::Result<String> {
    let output = Command::new(LSB_RELEASE_CMD)
        .arg("--short")
        .arg("--codename")
        .output();

    let stdout = match output {
        Ok(output) => output.stdout,
        // The `lsb_release` binary wasn't on the `PATH`.
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            bail!(Error::CouldNotInferSuite)
        }
        Err(err) => {
            return Err(err).wrap_err(format!(
                "failed getting distro version codename with `{}`",
                LSB_RELEASE_CMD
            ))
        }
    };

    Ok(String::from_utf8(stdout)?.trim().to_string())
}

fn parse_etc_os_release(file: &mut File) -> eyre::Result<String> {
    for line_result in BufReader::new(file).lines() {
        let line = line_result.wrap_err(format!("error reading {} file", OS_RELEASE_PATH))?;

        let (key, value) = match line.split_once('=') {
            Some((key, value)) => (key.trim(), value.trim()),
            None => continue,
        };

        if key == VERSION_CODENAME_KEY {
            return Ok(value.to_string());
        }
    }

    Err(eyre!(Error::CouldNotInferSuite))
}

/// Return the current distro version codename by parsing /etc/os-release.
fn from_etc_os_release() -> eyre::Result<String> {
    let os_release_path = Path::new(OS_RELEASE_PATH);

    if !os_release_path.exists() {
        bail!(Error::CouldNotInferSuite)
    }

    let mut os_release_file = File::open(os_release_path)?;

    parse_etc_os_release(&mut os_release_file)
}

/// Get the current distro version codename.
pub fn get_version_codename() -> eyre::Result<String> {
    match from_lsb_release() {
        Ok(codename) => Ok(codename),
        Err(err) => match err.downcast::<Error>() {
            Ok(err) if err == Error::CouldNotInferSuite => from_etc_os_release(),
            Ok(err) => Err(eyre!(err)),
            Err(err) => Err(err),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Seek, SeekFrom, Write};

    use xpct::{be_err, be_ok, equal, expect};

    use crate::error::Error;

    use super::*;

    // This is just what I happen to have on my machine.
    const OS_RELEASE: &str = r#"
        NAME="Pop!_OS"
        VERSION="22.04 LTS"
        ID=pop
        ID_LIKE="ubuntu debian"
        PRETTY_NAME="Pop!_OS 22.04 LTS"
        VERSION_ID="22.04"
        HOME_URL="https://pop.system76.com"
        SUPPORT_URL="https://support.system76.com"
        BUG_REPORT_URL="https://github.com/pop-os/pop/issues"
        PRIVACY_POLICY_URL="https://system76.com/privacy"
        VERSION_CODENAME=jammy
        UBUNTU_CODENAME=jammy
        LOGO=distributor-logo-pop-os
    "#;

    const MISSING_VERSION_OS_RELEASE: &str = r#"
        NAME="Pop!_OS"
        VERSION="22.04 LTS"
        ID=pop
        ID_LIKE="ubuntu debian"
        PRETTY_NAME="Pop!_OS 22.04 LTS"
        VERSION_ID="22.04"
        HOME_URL="https://pop.system76.com"
        SUPPORT_URL="https://support.system76.com"
        BUG_REPORT_URL="https://github.com/pop-os/pop/issues"
        PRIVACY_POLICY_URL="https://system76.com/privacy"
        LOGO=distributor-logo-pop-os
    "#;

    #[test]
    fn parses_os_release() -> eyre::Result<()> {
        let mut file = tempfile::tempfile()?;

        file.write_all(OS_RELEASE.as_bytes())?;
        file.seek(SeekFrom::Start(0))?;

        expect!(parse_etc_os_release(&mut file))
            .to(be_ok())
            .to(equal("jammy"));

        Ok(())
    }

    #[test]
    fn fails_when_version_codename_is_missing() -> eyre::Result<()> {
        let mut file = tempfile::tempfile()?;

        file.write_all(MISSING_VERSION_OS_RELEASE.as_bytes())?;
        file.seek(SeekFrom::Start(0))?;

        expect!(parse_etc_os_release(&mut file))
            .to(be_err())
            .map(|err| err.downcast::<Error>())
            .to(be_ok())
            .to(equal(Error::CouldNotInferSuite));

        Ok(())
    }
}
