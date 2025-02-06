/// Things about downloading.

use std::{io::Write, marker::PhantomData, ops::Add, os::unix::fs::MetadataExt};

use anyhow::Result;
use futures_util::{future::join_all, io::AllowStdIo, StreamExt};

use reqwest::IntoUrl;
use sha1::{digest::{generic_array::ArrayLength, OutputSizeUser}, Digest, Sha1};
use tokio::{fs::{self, File}, io::{AsyncWrite, AsyncWriteExt}, sync::{mpsc, Semaphore}};
use tokio_util::compat::TokioAsyncReadCompatExt;

use crate::minecraft::schemas::Resource;
use super::BetterPath;

/// Check the hash of a file.
/// 
/// # Arguments
/// * `T` - A hash algorithm like [Sha1] or [Sha256](sha2::Sha256)
pub async fn check_hash<T: Digest + Write>(path: &BetterPath, digest: &str, size: usize, _: PhantomData<T>) -> bool
    where <T as OutputSizeUser>::OutputSize: Add, <<T as OutputSizeUser>::OutputSize as Add>::Output: ArrayLength<u8> {
    let meta = fs::metadata(path).await;
    if meta.is_err() {
        return false;
    }
    let meta = meta.unwrap();
    if size != 0 && meta.size() as usize != size {
        return false;
    }
    if let Ok(f) = File::open(path).await {
        let mut sha1 = AllowStdIo::new(T::new());
        if futures_util::io::copy(&mut f.compat(), &mut sha1).await.is_err() {
            return false;
        }
        let out = sha1.into_inner().finalize();
        format!("{out:x}") == digest
    } else {
        false
    }
}

/// Download a [Resource] to the `path`.
pub async fn download_res(res: &Resource, path: &BetterPath) -> Result<()> {
    if check_hash(path, &res.sha1, res.size, PhantomData::<Sha1>).await {
        return Ok(());
    }
    download(res.url.clone(), path).await
}

/// Messages for download_all in channel.
#[derive(Clone)]
pub enum DownloadAllMessage {
    /// Download has started with these urls.
    Started(Vec<String>),
    /// This url is finished.
    SingleFinished(String),
    /// This resource(1) is finished with error(2)
    SingleError(Resource, String)
}

/// Download [Resource]s to paths.
pub async fn download_all(resources: &Vec<(Resource, BetterPath)>, channel: mpsc::UnboundedSender<DownloadAllMessage>) -> Result<()> {
    let semaphore = Semaphore::new(16);
    let mut futures = Vec::new();
    channel.send(DownloadAllMessage::Started(resources.iter().map(|v|v.0.url.clone()).collect()))?;
    for (res, path) in resources {
        futures.push(download_res_helper(res, path, channel.clone(), &semaphore));
    }
    for result in join_all(futures).await {
        if result.is_err() {
            // channel.closed().await;
            return result;
        }
    }
    // channel.closed().await;
    Ok(())
}

async fn download_res_helper(res: &Resource, path: &BetterPath, channel: mpsc::UnboundedSender<DownloadAllMessage>, semaphore: &Semaphore) -> Result<()> {
    let _ = semaphore.acquire().await?;
    let v = download_res(res, path).await;
    if let Err(e) = &v {
        channel.clone().send(DownloadAllMessage::SingleError(res.clone(), format!("{e}")))?;
    } else {
        channel.clone().send(DownloadAllMessage::SingleFinished(res.url.clone()))?;
    }
    v
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
pub async fn download<URL: IntoUrl>(url: URL, path: &BetterPath) -> Result<()> {
    if let Some(p) = path.0.parent() {
        fs::create_dir_all(p).await?;
    }
    let mut file = File::create(path).await?;

    download_to_writer(url, &mut file).await
}

/// Download the `url` into the `path`, and return the content.
pub async fn download_txt<'o, URL: IntoUrl>(url: URL, path: &BetterPath) -> Result<String> {
    let txt = reqwest::get(url).await?.text().await?;
    if let Some(p) = path.0.parent() {
        fs::create_dir_all(p).await?;
    }
    fs::write(path, &txt).await?;
    Ok(txt)
}