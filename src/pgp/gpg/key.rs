use std::io::BufRead;
use std::process::Stdio;

use eyre::{bail, WrapErr};

use crate::pgp::{KeyEncoding, KeyId};
use crate::stdio::{read_stderr, read_stdout, wait, write_stdin};

use super::client::GnupgClient;

/// The machine-readable output of a GnuPG command.
#[derive(Debug)]
struct ColonOutput {
    lines: Vec<Vec<String>>,
}

impl ColonOutput {
    const RECORD_TYPE_INDEX: usize = 0;
    const KEY_ID_INDEX: usize = 4;

    /// Create a new instance from a gpg command's stdout.
    pub fn new(output: &[u8]) -> eyre::Result<Self> {
        let mut lines = Vec::new();

        for line_result in output.lines() {
            let line = line_result.wrap_err("error decoding command output")?;

            lines.push(line.split(':').map(ToString::to_string).collect::<Vec<_>>());
        }

        Ok(Self { lines })
    }

    /// Get the key ID of the public key.
    pub fn public_key_id(&self) -> eyre::Result<KeyId> {
        for line in &self.lines {
            let record_type = match line.get(Self::RECORD_TYPE_INDEX) {
                Some(record_type) => record_type,
                None => bail!("could not find record type in gpg colon output"),
            };

            if record_type != "pub" {
                continue;
            }

            match line.get(Self::KEY_ID_INDEX) {
                Some(key_id) => return Ok(KeyId::new(key_id.to_string())),
                None => bail!("could not find key id in gpg colon output"),
            }
        }

        bail!("could not find public key record in gpg colon output");
    }
}

impl GnupgClient {
    /// Create a new PGP key.
    pub(super) fn new_key(
        &self,
        bytes: Vec<u8>,
        encoding: KeyEncoding,
        id: Option<KeyId>,
    ) -> eyre::Result<GnupgKey> {
        Ok(GnupgKey {
            client: self.clone(),
            bytes,
            encoding,
            id,
        })
    }
}

/// A PGP key specific to the GnuPG implementation.
#[derive(Debug)]
pub struct GnupgKey {
    client: GnupgClient,
    bytes: Vec<u8>,
    encoding: KeyEncoding,
    id: Option<KeyId>,
}

impl GnupgKey {
    /// Dearmor this key.
    pub fn dearmor(self) -> eyre::Result<Self> {
        if self.encoding == KeyEncoding::Binary {
            return Ok(self);
        }

        let mut process = self
            .client
            .command()
            .arg("--dearmor")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| self.client.map_err(err))?;

        let stdout_handle = read_stdout(&mut process);
        let stderr_handle = read_stderr(&mut process);

        write_stdin(&mut process, &mut self.bytes.as_slice())?;

        wait(process, stderr_handle)?;

        let dearmored_key = stdout_handle.join()?;

        Ok(Self {
            client: self.client,
            bytes: dearmored_key,
            encoding: KeyEncoding::Binary,
            id: self.id,
        })
    }

    /// Armor this key.
    pub fn enarmor(mut self) -> eyre::Result<Self> {
        if self.encoding == KeyEncoding::Armored {
            return Ok(self);
        }

        let mut keyring = self
            .client
            .new_keyring()
            .wrap_err("failed creating keyring")?;

        let keyring_key = keyring
            .import(&mut self)
            .wrap_err("failed importing key into keyring")?;

        keyring
            .export(keyring_key, KeyEncoding::Armored)
            .wrap_err("failed exporting key from keyring")
    }

    /// Return the key's key ID.
    pub fn id(&mut self) -> eyre::Result<KeyId> {
        if let Some(id) = &self.id {
            return Ok(id.clone());
        }

        let mut process = self
            .client
            .command()
            .arg("--show-keys")
            .arg("--with-colons")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| self.client.map_err(err))?;

        let stdout_handle = read_stdout(&mut process);
        let stderr_handle = read_stderr(&mut process);

        write_stdin(&mut process, &mut self.bytes.as_slice())?;

        wait(process, stderr_handle)?;

        let command_output = stdout_handle.join()?;

        let key_id = ColonOutput::new(&command_output)?
            .public_key_id()
            .wrap_err("failed parsing gpg output")?;

        self.id = Some(key_id.clone());

        Ok(key_id)
    }

    /// Consume this key and return its bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl AsRef<[u8]> for GnupgKey {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}
