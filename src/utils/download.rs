/// Things about downloading.
use std::{os::unix::fs::MetadataExt, path::{Path, PathBuf}, sync::Arc};


use futures_util::{io::AllowStdIo, TryStreamExt};

use reqwest::IntoUrl;
use sha1::{Digest, Sha1, digest::Update};
use tokio::{fs::{self, File}, io::{AsyncWrite, AsyncWriteExt}, sync::{Semaphore, mpsc::{self}}, task::JoinSet};
use std::fs as sync_fs;

use crate::{minecraft::schemas::Resource};

/// Check the hash of a file.
///
/// # Arguments
/// * `T` - A hash algorithm like [Sha1] or [Sha256](sha2::Sha256)
pub fn check_hash<T: Digest + Update>(path: impl AsRef<Path>, digest: Option<&str>, size: usize) -> bool {
    let meta = sync_fs::metadata(&path);
    if meta.is_err() {
        return false;
    }
    let meta = meta.unwrap();
    if size != 0 && meta.size() as usize != size {
        return false;
    }
    if digest.is_none() {
        return true;
    }
    if let Ok(mut f) = sync_fs::File::open(path) {
        let mut hash = AllowStdIo::new(digest_io::IoWrapper(T::new()));
        if std::io::copy(&mut f, &mut hash).is_err() {
            return false;
        }
        let res = hex::encode(hash.into_inner().0.finalize());
        &res == digest.unwrap()
    } else {
        false
    }
}

/// Download a [Resource] to the `path`.
pub async fn download_res(res: &Resource, path: &Path) -> Result<()> {
    if check_hash::<Sha1>(path, res.sha1.as_deref(), res.size) {
        return Ok(());
    }
    download(res.url.clone(), path).await
}

/// Events for single file.
#[derive(Debug)]
pub enum DownloadEvent {
    /// Download has been started
    Start,
    /// File size known
    ContentLength(u64),
    /// New chunk arrived with size
    Chunk(u64),
    /// File download retrying
    Retry(DownloadError),
    /// Download finished
    Finish(Result<()>),
}

/// Messages for download_all in channel.
pub type DownloadAllMessage = (String, DownloadEvent);

async fn check_and_download(path: impl AsRef<Path>, res: Resource, name: String, retries: usize, tx: mpsc::Sender<DownloadAllMessage>) {
    if check_hash::<Sha1>(&path, res.sha1.as_deref(), res.size) {
        return;
    }
    let _ = std::fs::create_dir_all(path.as_ref().parent().unwrap());
    let _ = tx.send((name.clone(), DownloadEvent::Start)).await;
    for time in 0..retries {
        match download_prog(res.url.clone(), &path, {
            let name = name.clone();
            let tx = tx.clone();
            async move |prog| {
                let name = name.clone();
                let _ = tx.send((name, prog)).await;
            }
        }).await {
            Ok(_) => {
                let _ = tx.send((name.clone(), DownloadEvent::Finish(Ok(())))).await;
                return;
            },
            Err(e) if time != retries - 1 => {
                let _ = tx.send((name.clone(), DownloadEvent::Retry(e))).await;
            }
            Err(e) => {
                let _ = tx.send((name.clone(), DownloadEvent::Finish(Err(e)))).await;
            }
        }
    }
}

/// Download [Resource]s to paths.
pub async fn download_all(
    files: Vec<(Resource, PathBuf, String)>,
    tx: mpsc::Sender<DownloadAllMessage>,
    parallel_files: usize, retries: usize,
    mirror: Option<String>
) {
    let mut set = JoinSet::new();
    let sema = Arc::new(Semaphore::new(parallel_files));
    for file in files {
        let sema = sema.clone();
        let tx = tx.clone();
        let resource = Resource {
            url: match &mirror {
                Some(mirror) => mirrored(&file.0.url, mirror),
                None => file.0.url
            },
            ..file.0
        };
        set.spawn(async move {
            let res = sema.acquire().await;
            check_and_download(file.1, resource, file.2, retries, tx).await;
            drop(res);
        });
    }
    set.join_all().await;
}

fn mirrored(url: &str, mirror: &str) -> String {
    url
        .replace("resources.download.minecraft.net", &format!("{mirror}/assets"))
        .replace("launchermeta.mojang.com", &mirror)
        .replace("launcher.mojang.com", &mirror)
        .replace("libraries.minecraft.net", &format!("{mirror}/maven"))
        .replace("files.minecraftforge.net", &mirror)
        .replace("maven.fabricmc.net", &format!("{mirror}/maven"))
        .replace("maven.neoforged.net/releases/net/neoforged/neoforge", &format!("{mirror}/maven/net/neoforged/neoforge"))
}

/// Read the `url` into the `writer`.
pub async fn download_to_writer<URL: IntoUrl, W: AsyncWrite + std::marker::Unpin>(url: URL, writer: &mut W) -> Result<()> {
    download_to_writer_prog(url, writer, async |_|()).await
}

/// Read the `url` into the `writer`.
pub async fn download<URL: IntoUrl>(url: URL, path: impl AsRef<Path>) -> Result<()> {
    download_prog(url, path, async |_|()).await
}

/// Read the `url` into the `writer`.
pub async fn download_to_writer_prog<URL: IntoUrl, W: AsyncWrite + std::marker::Unpin>(url: URL, writer: &mut W, cb: impl AsyncFn(DownloadEvent) -> () + Send) -> Result<()> {
    let resp = reqwest::get(url).await?;
    if let Some(l) = resp.content_length() {
        cb(DownloadEvent::ContentLength(l)).await;
    }

    let writer = resp.bytes_stream()
        .map_err(|err|DownloadError::from(err))
        .try_fold(writer, async |writer, bytes| {
            writer.write(&bytes).await?;
            cb(DownloadEvent::Chunk(bytes.len() as u64)).await;
            Ok(writer)
        }).await?;

    writer.flush().await?;
    Ok(())
}

/// Download the `url` into the `path`.
pub async fn download_prog<URL: IntoUrl>(url: URL, path: impl AsRef<Path>, cb: impl AsyncFn(DownloadEvent) -> () + Send) -> Result<()> {
    if let Some(p) = path.as_ref().parent() {
        fs::create_dir_all(p).await?;
    }
    let mut file = File::create(path).await?;

    download_to_writer_prog(url, &mut file, cb).await
}

/// Download the `url` into the `path`, and return the content.
pub async fn download_txt<URL: IntoUrl>(url: URL, path: impl AsRef<Path>) -> Result<String> {
    let txt = reqwest::get(url).await?.text().await?;
    if let Some(p) = path.as_ref().parent() {
        fs::create_dir_all(p).await?;
    }
    std::fs::write(path, &txt)?;
    Ok(txt)
}

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    #[error("Network Error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("IO Error: {0}")]
    IOError(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, DownloadError>;
