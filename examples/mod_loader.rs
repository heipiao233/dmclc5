use std::{path::{Path, PathBuf}, str::FromStr};

use dmclc5::{minecraft::schemas::VersionList, utils::{download, BetterPath, DownloadAllMessage}, LauncherContext, StdioUserInterface};
use tokio::sync::mpsc;

async fn handle_msg(msg: DownloadAllMessage) {
    match msg {
        DownloadAllMessage::Started(urls) => {
            println!("Download starts with: {}", urls.join("\n"));
        }
        DownloadAllMessage::SingleFinished(url) => {
            println!("{} finished.", url);
        }

        DownloadAllMessage::SingleError(res, error) => {
            println!("{} finished with error {}.", res.url, error);
        }
    }
}

#[tokio::main]
async fn main() {
    let launcher: LauncherContext = LauncherContext::new(Path::new("./test"), StdioUserInterface).await.unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let handler = async move {
        while let Some(next) = rx.recv().await {
            handle_msg(next).await;
        }
    };
    let mc = VersionList::get_list().await.unwrap();
    let mc = mc.find_by_id("1.20.4").unwrap().install(&launcher, "1.20.4-fabric", tx);
    let mut mc = tokio::join!(handler, mc).1.unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let handler = async move {
        while let Some(next) = rx.recv().await {
            handle_msg(next).await;
        }
    };
    tokio::join!(mc.install_component("fabric", "0.16.0", tx), handler).0.unwrap();
    let path = BetterPath(PathBuf::from_str("./test/versions/1.20.4-fabric/mods/entityculling-fabric-1.6.6-mc1.20.4.jar").unwrap());
    download("https://cdn.modrinth.com/data/NNAgCjsB/versions/cj8nR3eG/entityculling-fabric-1.6.6-mc1.20.4.jar", &path).await.unwrap();
    println!("{:#?}", mc.list_mods().await.unwrap());
    println!("{:#?}", mc.check_mod_dependencies().await.unwrap());
}
