use std::sync::Arc;

use serde_yaml::Error as YamlError;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum Error {
    #[error("Failed parsing the config: {0}")]
    ParsingConfigError(#[from] Arc<YamlError>),

    #[error("Invalid config: {0}")]
    InvalidConfigError(String),

    #[error("IO Error: {0}")]
    IO(#[from] Arc<std::io::Error>),
}

/// The result type used in this crate.
pub type Result<T> = std::result::Result<T, Error>;
