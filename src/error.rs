use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("This option name is invalid: {name}")]
    InvalidOptionName { name: String },

    #[error("This option is not in `key=value` format: {option}")]
    MalformedOption { option: String },
}
