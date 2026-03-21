use thiserror::Error;

#[derive(Error, Debug)]
pub enum SqlcxError {
    #[error("config file not found: {0}")]
    ConfigNotFound(String),

    #[error("invalid config: {0}")]
    ConfigInvalid(String),

    #[error("SQL parse error in {file}: {message}")]
    ParseError { file: String, message: String },

    #[error("unknown column type: {0}")]
    UnknownType(String),

    #[error("missing query annotation in {file}")]
    MissingAnnotation { file: String },

    #[error("unknown parser: {0}")]
    UnknownParser(String),

    #[error("unknown language: {0}")]
    UnknownLanguage(String),

    #[error("unknown schema generator: {0}")]
    UnknownSchema(String),

    #[error("unknown driver generator: {0}")]
    UnknownDriver(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, SqlcxError>;
