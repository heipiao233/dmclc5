use std::{path::{Path, PathBuf}, str::FromStr};

use dmclc5::{minecraft::schemas::VersionList, utils::{download, BetterPath}, LauncherContext, StdioUserInterface};

#[tokio::main]
async fn main() {
    let launcher: LauncherContext = LauncherContext::new(Path::new("./test"), StdioUserInterface).await.unwrap();
    let mut mc = VersionList::get_list().await.unwrap().find_by_id("1.20.4").unwrap().install(&launcher, "1.20.4-fabric").await.unwrap();
    mc.install_component("fabric", "0.16.0").await.unwrap();
    let path = BetterPath(PathBuf::from_str("./test/versions/1.20.4-fabric/mods/entityculling-fabric-1.6.6-mc1.20.4.jar").unwrap());
    download("https://cdn.modrinth.com/data/NNAgCjsB/versions/cj8nR3eG/entityculling-fabric-1.6.6-mc1.20.4.jar", &path).await.unwrap();
    println!("{:#?}", mc.list_mods().await.unwrap());
    println!("{:#?}", mc.check_mod_dependencies().await.unwrap());
}
