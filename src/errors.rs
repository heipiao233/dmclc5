#[derive(thiserror::Error, Debug)]
pub enum LauncherError {
    #[error("Network Error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("JSON Serialize/Deserialize Error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("IO Error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Zip File Error: {0}")]
    ZipError(#[from] zip::result::ZipError),
    #[error("Download Error: {0}")]
    DownloadError(#[from] crate::utils::download::DownloadError),
    #[error("Account Error: {0}")]
    AccountError(#[from] crate::minecraft::login::AccountError),
}

pub type Result<T> = std::result::Result<T, LauncherError>;
