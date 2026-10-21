use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{0} is protected: {1}")]
    Protected(PathBuf, &'static str),
    #[error("{0} is not inside the scanned folder")]
    NotInTree(PathBuf),
    #[error("scan was cancelled")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io { path: path.into(), source }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
