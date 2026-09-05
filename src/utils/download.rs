/// Things about downloading.
use std::{os::unix::fs::MetadataExt, path::{Path, PathBuf}, sync::Arc, time::Duration};

use anyhow::Result;
use futures_util::{io::AllowStdIo, StreamExt, TryStreamExt};

use reqwest::IntoUrl;
use sha1::{Digest, Sha1, digest::Update};
use tokio::{fs::{self, File}, io::{AsyncWrite, AsyncWriteExt}, sync::{Semaphore, mpsc::{self, UnboundedReceiver}, watch}, task::JoinSet};
use std::fs as sync_fs;

use crate::minecraft::schemas::Resource;

/// Check the hash of a file.
///
/// # Arguments
/// * `T` - A hash algorithm like [Sha1] or [Sha256](sha2::Sha256)
pub fn check_hash<T: Digest + Update>(path: impl AsRef<Path>, digest: &str, size: usize) -> bool {
    let meta = sync_fs::metadata(&path);
    if meta.is_err() {
        return false;
    }
    let meta = meta.unwrap();
    if size != 0 && meta.size() as usize != size {
        return false;
    }
    if let Ok(mut f) = sync_fs::File::open(path) {
        let mut hash = AllowStdIo::new(digest_io::IoWrapper(T::new()));
        if std::io::copy(&mut f, &mut hash).is_err() {
            return false;
        }
        hex::encode(hash.into_inner().0.finalize()) == digest
    } else {
        false
    }
}

/// Download a [Resource] to the `path`.
pub async fn download_res(res: &Resource, path: &Path) -> Result<()> {
    if check_hash::<Sha1>(path, &res.sha1, res.size) {
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
    /// Download progress
    Progress(u64),
    /// File download retrying
    Retry(anyhow::Error),
    /// Download finished
    Finish(anyhow::Result<()>),
}

/// Messages for download_all in channel.
pub type DownloadAllMessage = (String, DownloadEvent);

async fn check_and_download(path: impl AsRef<Path>, res: Resource, name: String, retries: usize, tx: mpsc::Sender<DownloadAllMessage>) {
    if check_hash::<Sha1>(&path, &res.sha1, res.size) {
        return;
    }
    let _ = std::fs::create_dir_all(path.as_ref().parent().unwrap());
    tx.send((name.clone(), DownloadEvent::Start));
    for time in 0..retries {
        match download_prog(res.url.clone(), &path, |prog| {
            tx.send((name.clone(), prog));
        }).await {
            Ok(_) => {
                tx.send((name.clone(), DownloadEvent::Finish(Ok(())))).await;
                return;
            },
            Err(e) if time != retries - 1 => {
                tx.send((name.clone(), DownloadEvent::Retry(e))).await;
            }
            Err(e) => {
                tx.send((name.clone(), DownloadEvent::Finish(Err(e)))).await;
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
            sema.acquire().await;
            check_and_download(file.1, resource, file.2, retries, tx);
        });
    }
}

fn mirrored(url: &str, mirror: &str) -> String {
    url
        .replace("resources.download.minecraft.net", &format!("{mirror}/assets"))
        .replace("libraries.minecraft.net", &format!("{mirror}/maven"))
        .replace("files.minecraftforge.net", &mirror)
        .replace("maven.fabricmc.net", &mirror)
        .replace("maven.neoforged.net/releases/net/neoforged/neoforge", &format!("{mirror}/maven/net/neoforged/neoforge"))
        .replace("resources.download.minecraft.net", &format!("{mirror}/assets"))
}

/// Read the `url` into the `writer`.
pub async fn download_to_writer<URL: IntoUrl, W: AsyncWrite + std::marker::Unpin>(url: URL, writer: &mut W) -> Result<()> {
    download_to_writer_prog(url, writer, |_|()).await
}

/// Read the `url` into the `writer`.
pub async fn download<URL: IntoUrl>(url: URL, path: impl AsRef<Path>) -> Result<()> {
    download_prog(url, path, |_|()).await
}

/// Read the `url` into the `writer`.
pub async fn download_to_writer_prog<URL: IntoUrl, W: AsyncWrite + std::marker::Unpin>(url: URL, writer: &mut W, cb: impl Fn(DownloadEvent) -> ()) -> Result<()> {
    let resp = reqwest::get(url).await?;
    resp.content_length().map(|l| cb(DownloadEvent::ContentLength(l)));

    let (writer, _) = resp.bytes_stream()
        .try_fold((writer, 0u64), async |(writer, prog), byte| {
            writer.write(&byte).await;
            let prog = prog + byte.len() as u64;
            cb(DownloadEvent::Progress(prog));
            Ok((writer, prog))
        }).await?;

    writer.flush().await?;
    Ok(())
}

/// Download the `url` into the `path`.
pub async fn download_prog<URL: IntoUrl>(url: URL, path: impl AsRef<Path>, cb: impl Fn(DownloadEvent) -> ()) -> Result<()> {
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
