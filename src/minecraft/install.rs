//! Things about installing Minecraft.

use std::{slice::Iter, sync::Arc, vec};

use anyhow::{Ok, Result};
use sha1::Sha1;
use tokio::{fs, io::AsyncReadExt, sync::mpsc};

use crate::{utils::{check_hash, check_rules, download_all, download_txt, get_os, BetterPathBuf, DownloadAllMessage}, LauncherContext};

use super::{schemas::{AssetsIndex, Library, Resource, VersionJSON}, version::{DMCLCExtraData, MinecraftInstallation}};
/// The version list of Minecraft.
pub use super::schemas::{VersionList, VersionInfo};

const MC_MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest.json";

impl VersionList {
    /// Gets the [VersionList].
    pub async fn get_list() -> Result<VersionList> {
        Ok(reqwest::get(MC_MANIFEST_URL)
            .await?
            .json()
            .await?)
    }

    /// Find a version by `id` in the [VersionList].
    pub fn find_by_id(&self, id: &str) -> Option<&VersionInfo> {
        self.iter().find(|i|i.id == id)
    }

    /// Returns an iterator of [VersionInfo]
    pub fn iter(&self) -> Iter<'_, VersionInfo> {
        self.versions.iter()
    }

    /// Get the latest release in the [VersionList].
    pub fn get_latest_release(&self) -> Option<&VersionInfo> {
        self.find_by_id(&self.latest.release)
    }

    /// Get the latest snapshot in the [VersionList].
    pub fn get_latest_snapshot(&self) -> Option<&VersionInfo> {
        self.find_by_id(&self.latest.release)
    }
}

impl IntoIterator for VersionList {
    type IntoIter = vec::IntoIter<VersionInfo>;
    type Item = VersionInfo;

    fn into_iter(self) -> Self::IntoIter {
        self.versions.into_iter()
    }
}

impl VersionInfo {
    /// Install
    pub async fn install<'l>(&self, launcher: Arc<LauncherContext>, name: &str, channel: mpsc::UnboundedSender<DownloadAllMessage>) -> Result<MinecraftInstallation> {
        let res = reqwest::get(&self.url).await?;
        let text = res.text().await?;
        let obj: VersionJSON = serde_json::from_str(&text)?;
        let version_dir = launcher.root_path.clone() / "versions" / name;
        fs::create_dir_all(version_dir.clone()).await?;
        fs::write(version_dir / format!("{name}.json"), text).await?;
        let v = MinecraftInstallation::new(launcher, obj, name, Some(DMCLCExtraData {
            version: Some(self.id.clone()),
            components: vec![],
            independent_game_dir: true,
            before_command: None,
            with_java: None,
            extra_game_arguments: None,
            extra_jvm_arguments: None
        }));
        v.complete_files(true, true, channel).await?;
        Ok(v)
    }
}

impl MinecraftInstallation {
    /// Download all the broken/missing files for the [MinecraftInstallation].
    pub async fn complete_files(&self, always_download_nohash: bool, fix_client_jar: bool, channel: mpsc::UnboundedSender<DownloadAllMessage>) -> Result<()> {
        let mut resources: Vec<(Resource, BetterPathBuf)> = Vec::new();
        let client_res = &self.obj.get_base().downloads.client;
        let version_dir = self.launcher.root_path.clone() / "versions" / &self.name;
        if fix_client_jar { resources.push((client_res.clone(), version_dir / format!("{}.jar", self.name))); }
        resources.extend(self.assets().await?);
        resources.extend(self.libraries(&self.obj.get_base().libraries, always_download_nohash));
        download_all(
            resources, channel,
            self.launcher.download_threads_per_file, self.launcher.download_parallel_files,
            self.launcher.download_retries,self.launcher.bmclapi_mirror.clone()
        ).await?;
        Ok(())
    }

    async fn assets(&self) -> Result<Vec<(Resource, BetterPathBuf)>> {
        let assets = &self.obj.get_base().asset_index;
        let asset_path = self.launcher.root_path.clone() / "assets/indexes" / format!("{}.json", assets.res.id);
        let index = if !check_hash::<Sha1>(asset_path.as_ref(), &assets.res.res.sha1, assets.res.res.size).await {
            download_txt(&assets.res.res.url, asset_path.as_ref()).await?
        } else {
            let mut str = String::new();
            tokio::fs::File::open(asset_path).await?.read_to_string(&mut str).await?;
            str
        };
        let index: AssetsIndex = serde_json::from_str(&index)?;
        Ok(index.objects.values()
            .map(|asset| (format!("{}/{}", &asset.hash[0..=1], asset.hash), asset))
            .map(|(path, asset)| (Resource {
                url: format!("https://resources.download.minecraft.net/{path}"),
                sha1: asset.hash.clone(),
                size: asset.size
            }, self.launcher.root_path.clone() / "assets/objects" / &path))
            .collect())
    }

    pub(crate) fn libraries(&self, libraries: &Vec<Library>, always_download_nohash: bool) -> Vec<(Resource, BetterPathBuf)> {
        let lib_path = self.launcher.root_path.clone() / "libraries";
        libraries.iter()
            .filter(|l| check_rules(&l.get_base().rules))
            .filter_map(|lib|
                match lib {
                    Library::FabricWithHash(l) => {
                        Some((Resource {
                            url: format!("{}/{}", l.url, l.base.name.to_path()),
                            sha1: l.sha1.clone(),
                            size: l.size
                        }, lib_path.clone() / l.base.name.to_path()))
                    },
                    Library::FabricOldForgeAndLiteLoader(l) if l.clientreq => {
                        Some((Resource {
                            url: format!("{}/{}", l.url, l.base.name.to_path()),
                            sha1: always_download_nohash.to_string(),
                            size: 0
                        }, lib_path.clone() / l.base.name.to_path()))
                    }
                    Library::VanillaForgeAndNeo(l) => {
                        Some((l.downloads.artifact.res.clone(), lib_path.clone() / &l.downloads.artifact.path))
                    }
                    Library::VanillaNatives(l) if let Some(os) = l.natives.get(&get_os()) => {
                        let artifact = l.downloads.classifiers.get(os).unwrap();
                        Some((artifact.res.clone(), lib_path.clone() / &artifact.path))
                    }
                    Library::BaseOnly(l) => {
                        Some((Resource {
                            url: format!("https://libraries.minecraft.net/{}", l.name.to_path()),
                            sha1: always_download_nohash.to_string(),
                            size: 0
                        }, lib_path.clone() / l.name.to_path()))
                    }
                    _ => None
                })
            .collect()
    }
}
