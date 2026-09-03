/// Things about downloading.
use std::{os::unix::fs::MetadataExt, path::{Path, PathBuf}, sync::Arc, time::Duration};

use anyhow::Result;
use async_fetcher::{FetchEvent, Fetcher, Source};
use futures_util::{io::AllowStdIo, StreamExt};

use reqwest::IntoUrl;
use sha1::{Digest, Sha1, digest::Update};
use tokio::{fs::{self, File}, io::{AsyncWrite, AsyncWriteExt}, sync::mpsc};
use tokio_util::compat::TokioAsyncReadCompatExt;

use crate::minecraft::schemas::Resource;

/// Check the hash of a file.
///
/// # Arguments
/// * `T` - A hash algorithm like [Sha1] or [Sha256](sha2::Sha256)
pub async fn check_hash<T: Digest + Update>(path: impl AsRef<Path>, digest: &str, size: usize) -> bool {
    let meta = fs::metadata(&path).await;
    if meta.is_err() {
        return false;
    }
    let meta = meta.unwrap();
    if size != 0 && meta.size() as usize != size {
        return false;
    }
    if let Ok(f) = File::open(path).await {
        let mut hash = AllowStdIo::new(digest_io::IoWrapper(T::new()));
        if futures_util::io::copy(&mut f.compat(), &mut hash).await.is_err() {
            return false;
        }
        hex::encode(hash.into_inner().0.finalize()) == digest
    } else {
        false
    }
}

/// Download a [Resource] to the `path`.
pub async fn download_res(res: &Resource, path: &Path) -> Result<()> {
    if check_hash::<Sha1>(path, &res.sha1, res.size).await {
        return Ok(());
    }
    download(res.url.clone(), path).await
}

/// Messages for download_all in channel.
pub type DownloadAllMessage = std::result::Result<(PathBuf, FetchEvent), (PathBuf, anyhow::Error)>;

async fn check_and_download(path: impl AsRef<Path>, res: &Resource, urls: Arc<[Box<str>]>) -> Option<(Source, Arc<()>)> {
    if !check_hash::<Sha1>(&path, &res.sha1, res.size).await {
        let _ = fs::create_dir_all(&path.as_ref().parent().unwrap()).await;
        Some((Source {
            dest: Arc::from(path.as_ref()),
            urls,
            part: None
        }, Arc::new(())))
    } else {
        None
    }
}

/// Download [Resource]s to paths.
pub async fn download_all(
    resources: Vec<(Resource, PathBuf)>, channel: mpsc::UnboundedSender<DownloadAllMessage>,
    threads_per_file: u16, parallel_files: usize, retries: usize,
    mirror: Option<String>
) -> Result<()> {
    let sources = futures_util::stream::iter(resources)
        .map(move |(res, path)| {
            let mirrored = mirror.clone().map(|mirror| mirrored(&res.url, mirror));
            (res, path, mirrored)
        })
        .filter_map(async |(res, path, mirrored)| {
            if let Some(mirrored) = mirrored {
                let url = res.url.clone();
                check_and_download(path, &res, Arc::new([Box::from(url.as_str()), Box::from(mirrored.as_str())])).await
            } else {
                check_and_download(path, &res, Arc::new([Box::from(res.url.as_str())])).await
            }
        });
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut fetcher = Fetcher::default()
        .events(tx)
        .retries(retries as u16)
        .timeout(Duration::from_secs(15))
        .connections_per_file(threads_per_file)
        .build()
        .stream_from(sources, parallel_files * (threads_per_file as usize));
    let channel2 = channel.clone();
    let fetch_task = async move {
        while let Some((path, _, result)) = fetcher.next().await {
            if let Err(e) = result {
                let _ = tokio::fs::remove_file(&path).await;
                let _ = channel2.send(Err((path.to_path_buf(), e.into())));
            }
        }
    };
    let send_task = async move {
        while let Some((path, _, event)) = rx.recv().await {
            let _ = channel.send(Ok((path.to_path_buf(), event)));
        }
    };
    tokio::join!(fetch_task, send_task);
    Ok(())
}

fn mirrored(url: &str, mirror: String) -> String {
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
    let mut stream = reqwest::get(url).await?.bytes_stream();

    while let Some(chunk) = stream.next().await {
        writer.write_all(&chunk?).await?;
        writer.flush().await?
    }

    writer.flush().await?;
    Ok(())
}

/// Download the `url` into the `path`.
pub async fn download<URL: IntoUrl>(url: URL, path: impl AsRef<Path>) -> Result<()> {
    if let Some(p) = path.as_ref().parent() {
        fs::create_dir_all(p).await?;
    }
    let mut file = File::create(path).await?;

    download_to_writer(url, &mut file).await
}

/// Download the `url` into the `path`, and return the content.
pub async fn download_txt<URL: IntoUrl>(url: URL, path: impl AsRef<Path>) -> Result<String> {
    let txt = reqwest::get(url).await?.text().await?;
    if let Some(p) = path.as_ref().parent() {
        fs::create_dir_all(p).await?;
    }
    fs::write(path, &txt).await?;
    Ok(txt)
}
