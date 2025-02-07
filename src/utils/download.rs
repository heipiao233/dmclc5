/// Things about downloading.

use std::{io::Write, marker::PhantomData, ops::Add, os::unix::fs::MetadataExt, sync::Arc, time::Duration};

use anyhow::Result;
use async_fetcher::{FetchEvent, Fetcher, Source};
use futures_util::{io::AllowStdIo, StreamExt};

use reqwest::IntoUrl;
use sha1::{digest::{generic_array::ArrayLength, OutputSizeUser}, Digest, Sha1};
use tokio::{fs::{self, File}, io::{AsyncWrite, AsyncWriteExt}, sync::mpsc};
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
pub type DownloadAllMessage = std::result::Result<(BetterPath, FetchEvent), (BetterPath, anyhow::Error)>;

async fn check_and_download(path: &BetterPath, res: &Resource, urls: Arc<[Box<str>]>) -> Option<(Source, Arc<()>)> {
    if !check_hash(path, &res.sha1, res.size, PhantomData::<Sha1>).await {
        let _ = fs::create_dir_all(&path.0.parent().unwrap()).await;
        Some((Source {
            dest: Arc::from(path.0.as_path()),
            urls,
            part: None
        }, Arc::new(())))
    } else {
        None
    }
}

/// Download [Resource]s to paths.
pub async fn download_all(
    resources: &Vec<(Resource, BetterPath)>, channel: mpsc::UnboundedSender<DownloadAllMessage>,
    threads_per_file: u16, parallel_files: usize, retries: usize,
    mirror: Option<String>
) -> Result<()> {
    let mut check_futures = vec![];
    for (res, path) in resources {
        let urls: Arc<[Box<str>]>;
        if let Some(mirror) = &mirror {
            urls = Arc::new([Box::from(mirrored(res.url.clone(), mirror.clone()).as_str()), Box::from(res.url.as_str())]);
        } else {
            urls = Arc::new([Box::from(res.url.as_str())]);
        }
        check_futures.push(check_and_download(path, res, urls));
    }
    let sources: Vec<_> = futures_util::future::join_all(check_futures).await
        .into_iter()
        .filter(|v|v.is_some())
        .map(|v|v.unwrap()).collect();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut fetcher = Fetcher::default()
        .events(tx)
        .retries(retries as u16)
        .timeout(Duration::from_secs(15))
        .connections_per_file(threads_per_file)
        .build()
        .stream_from(futures_util::stream::iter(sources), parallel_files * (threads_per_file as usize));
    let channel2 = channel.clone();
    let fetch_task = async move {
        while let Some((path, _, result)) = fetcher.next().await {
            if let Err(e) = result {
                let _ = tokio::fs::remove_file(&path).await;
                let _ = channel2.send(Err((BetterPath::from(path.to_path_buf()), e.into())));
            }
        }
    };
    let send_task = async move {
        while let Some((path, _, event)) = rx.recv().await {
            let _ = channel.send(Ok((BetterPath::from(path.to_path_buf()), event)));
        }
    };
    tokio::join!(fetch_task, send_task);
    Ok(())
}

fn mirrored(url: String, mirror: String) -> String {
    return url
        .replace("resources.download.minecraft.net", &format!("{mirror}/assets"))
        .replace("libraries.minecraft.net", &format!("{mirror}/maven"))
        .replace("files.minecraftforge.net", &mirror)
        .replace("maven.fabricmc.net", &mirror)
        .replace("maven.neoforged.net/releases/net/neoforged/neoforge", &format!("{mirror}/maven/net/neoforged/neoforge"))
        .replace("resources.download.minecraft.net", &format!("{mirror}/assets"));
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