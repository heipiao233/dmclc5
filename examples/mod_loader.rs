use std::{path::PathBuf, str::FromStr};

use dmclc5::{LauncherConfig, components::install::fabriclike::FABRIC_INSTALLER, minecraft::{prefix::MinecraftPrefix, schemas::VersionList}, utils::{DownloadAllMessage, DownloadEvent, download}};
use tokio::sync::mpsc;

async fn handle_msg(msg: DownloadAllMessage, count: &mut usize) {
    match msg {
        (c, DownloadEvent::ContentLength(len)) => {
            println!("{} start: {len}", c);
        },
        (c, DownloadEvent::Finish(_)) => {
            println!("{} end ({count})", c);
            *count -= 1;
        },
        (_, DownloadEvent::Start) => {
            *count += 1;
            println!("{count}");
        },
        (_c, DownloadEvent::Progress(_prog)) => {
            // println!("{} fetching: {prog}", c.0.display());
        },
        (c, DownloadEvent::Retry(_)) => {
            println!("{} retrying", c);
        },
    }
}

#[tokio::main]
async fn main() {
    let config: LauncherConfig = LauncherConfig::new("dmclc example mod_loader".to_string(), PathBuf::from("./test/assets"), PathBuf::from("./test/libraries")).unwrap();
    let prefix: MinecraftPrefix = MinecraftPrefix::new(PathBuf::from("./test")).unwrap();
    let (tx, mut rx) = mpsc::channel(1000);
    let handler = async move {
        let mut count = 0;
        while let Some(next) = rx.recv().await {
            handle_msg(next, &mut count).await;
        }
    };
    let mc = VersionList::get_list().await.unwrap();
    let mc = mc.find_by_id("1.20.4").unwrap().install(&prefix, &config, "1.20.4-fabric", tx);
    let mut mc = tokio::join!(handler, mc).1.unwrap();
    let (tx, mut rx) = mpsc::channel(1000);
    let handler = async move {
        let mut count = 0;
        while let Some(next) = rx.recv().await {
            handle_msg(next, &mut count).await;
        }
    };
    tokio::join!(mc.install_component(FABRIC_INSTALLER, "0.16.0", tx), handler).0.unwrap();
    let path = PathBuf::from_str("./test/versions/1.20.4-fabric/mods/entityculling-fabric-1.6.6-mc1.20.4.jar").unwrap();
    download("https://cdn.modrinth.com/data/NNAgCjsB/versions/cj8nR3eG/entityculling-fabric-1.6.6-mc1.20.4.jar", &path).await.unwrap();
    println!("{:#?}", mc.list_mods().await.unwrap());
    println!("{:#?}", mc.check_mod_dependencies().await.unwrap());
}
