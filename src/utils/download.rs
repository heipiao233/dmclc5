/// Things about downloading.

use std::{io::Write, marker::PhantomData, ops::Add, os::unix::fs::MetadataExt};

use anyhow::Result;
use futures_util::{future::join_all, io::AllowStdIo, StreamExt};

use reqwest::IntoUrl;
use sha1::{digest::{generic_array::ArrayLength, OutputSizeUser}, Digest, Sha1};
use tokio::{fs::{self, File}, io::{AsyncWrite, AsyncWriteExt}};
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

/// Download [Resource]s to paths.
pub async fn download_all(resources: &Vec<(Resource, BetterPath)>) -> Result<()> {
    let mut futures = Vec::new();
    for (res, path) in resources {
        futures.push(download_res(res, path));
    }
    for result in join_all(futures).await {
        if result.is_err() {
            return result;
        }
    }
    Ok(())
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