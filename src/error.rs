use thiserror::Error;
use tokio::io;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("UniFi error: {0}")]
    UniFi(#[from] unifi_rs::UnifiError),

    #[error("Application error: {0}")]
    Application(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<AppError> for io::Error {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Io(e) => e,
            _ => io::Error::new(io::ErrorKind::Other, error.to_string()),
        }
    }
}

pub type Result<T> = std::result::Result<T, AppError>;