use std::fs::File;

use eyre::bail;
use reqwest::Url;

use crate::error::Error;

/// Download a file from `url` and return its file handle.
pub fn download_file(url: &Url) -> eyre::Result<File> {
    let mut temp_file = tempfile::tempfile()?;

    let mut response = reqwest::blocking::get(url.clone())?;
    let status = response.status();

    if status.is_success() {
        response.copy_to(&mut temp_file)?;
    } else {
        bail!(Error::KeyDownloadFailed {
            url: url.to_string(),
            reason: match status.canonical_reason() {
                Some(reason_phrase) => format!("Error: {}", reason_phrase),
                None => format!("Error Code: {}", status.as_str()),
            }
        })
    }

    Ok(temp_file)
}
