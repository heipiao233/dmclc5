//! Errors from this crate's core functions.

/// Errors from this crate's core functions.
#[derive(thiserror::Error, Debug)]
pub enum LauncherError {
    /// Network error.
    #[error("Network Error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    /// Failure when (de)serializing JSON.
    #[error("JSON Serialize/Deserialize Error: {0}")]
    JsonError(#[from] serde_json::Error),
    /// IO error.
    #[error("IO Error: {0}")]
    IOError(#[from] std::io::Error),
    /// Failure when reading a ZIP file.
    #[error("Zip File Error: {0}")]
    ZipError(#[from] zip::result::ZipError),
    /// Failure when downloading.
    #[error("Download Error: {0}")]
    DownloadError(#[from] crate::utils::download::DownloadError),
    /// An error about accounts.
    #[error("Account Error: {0}")]
    AccountError(#[from] crate::minecraft::login::AccountError),
}

pub(crate) type Result<T> = std::result::Result<T, LauncherError>;
