//! Things about installing Minecraft.

use std::marker::PhantomData;

use anyhow::{Ok, Result};
use sha1::Sha1;
use tokio::{fs, io::AsyncReadExt};

use crate::{utils::{check_hash, check_rules, download_all, download_txt, get_os, BetterPath}, LauncherContext};

use super::{schemas::{AssetsIndex, Library, Resource, VersionInfo, VersionJSON}, version::{DMCLCExtraData, MinecraftInstallation}};
/// The version list of Minecraft.
pub use super::schemas::VersionList;

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
        self.versions.iter().find(|i|i.id == id)
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

impl VersionInfo {
    /// Install
    pub async fn install<'l>(&self, launcher: &'l LauncherContext, name: &str) -> Result<MinecraftInstallation<'l>> {
        let res = reqwest::get(&self.url).await?;
        let text = res.text().await?;
        let obj: VersionJSON = serde_json::from_str(&text)?;
        let version_dir = *(&launcher.root_path / "versions" / name);
        fs::create_dir_all(version_dir.clone()).await?;
        fs::write(&version_dir / format!("{name}.json"), text).await?;
        let v = MinecraftInstallation::<'l>::new(launcher, obj, name, Some(DMCLCExtraData {
            version: Some(self.id.clone()),
            components: vec![],
            independent_game_dir: true,
            before_command: None,
            with_java: None,
            extra_game_arguments: None,
            extra_jvm_arguments: None
        }));
        v.complete_files(true, true).await?;
        Ok(v)
    }
}

impl <'l> MinecraftInstallation<'l> {
    /// Download all the broken/missing files for the [MinecraftInstallation].
    pub async fn complete_files(&self, always_download_nohash: bool, fix_client_jar: bool) -> Result<()> {
        let mut resources: Vec<(Resource, BetterPath)> = Vec::new();
        let client_res = &self.obj.get_base().downloads.client;
        let version_dir = *(&self.launcher.root_path / "versions" / &self.name);
        if fix_client_jar { resources.push((client_res.clone(), *(&version_dir / format!("{}.jar", self.name)))); }
        self.install_resources(&mut resources).await?;
        self.install_libraries(&self.obj.get_base().libraries, &mut resources, always_download_nohash)?;
        download_all(&resources).await?;
        Ok(())
    }

    async fn install_resources(&self, res: &mut Vec<(Resource, BetterPath)>) -> Result<()> {
        let assets = &self.obj.get_base().asset_index;
        let asset_path = &(&self.launcher.root_path / "assets/indexes" / &format!("{}.json", assets.res.id));
        let index = if check_hash(asset_path, &assets.res.res.sha1, assets.res.res.size, PhantomData::<Sha1>).await {
            download_txt(&assets.res.res.url, asset_path).await?
        } else {
            let mut str = String::new();
            tokio::fs::File::open(asset_path).await?.read_to_string(&mut str).await?;
            str
        };
        let index: AssetsIndex = serde_json::from_str(&index)?;
        for (_, val) in index.objects.iter() {
            let first_two = val.hash.get(0..=1).unwrap(); // Must be ASCII.
            let path = format!("{first_two}/{}", val.hash);
            res.push((Resource {
                url: format!("https://resources.download.minecraft.net/{path}"),
                sha1: val.hash.clone(),
                size: val.size
            }, *(&self.launcher.root_path / "assets/objects" / &path)))
        }
        Ok(())
    }
    
    pub(crate) fn install_libraries(&self, libraries: &Vec<Library>, res: &mut Vec<(Resource, BetterPath)>, always_download_nohash: bool) -> Result<()> {
        let lib_path = &*(&self.launcher.root_path / "libraries");
        for lib in libraries {
            if !check_rules(&lib.get_base().rules) {
                continue;
            }
            match lib {
                Library::FabricWithHash(l) => {
                    res.push((Resource {
                        url: format!("{}/{}", l.url, l.base.name.to_path()),
                        sha1: l.sha1.clone(),
                        size: l.size
                    }, *(lib_path / l.base.name.to_path())));
                },
                Library::FabricOldForgeAndLiteLoader(l) => {
                    if !l.clientreq {
                        continue;
                    }
                    res.push((Resource {
                        url: format!("{}/{}", l.url, l.base.name.to_path()),
                        sha1: always_download_nohash.to_string(),
                        size: 0
                    }, *(lib_path / l.base.name.to_path())));
                }
                Library::VanillaForgeAndNeo(l) => {
                    res.push((l.downloads.artifact.res.clone(), *(lib_path / &l.downloads.artifact.path)))
                }
                Library::VanillaNatives(l) => {
                    if let Some(os) = l.natives.get(&get_os()) {
                        let artifact = l.downloads.classifiers.get(os).unwrap();
                        res.push((artifact.res.clone(), *(lib_path / &artifact.path)))
                    }
                }
                Library::BaseOnly(l) => {
                    res.push((Resource {
                        url: format!("https://libraries.minecraft.net/{}", l.name.to_path()),
                        sha1: always_download_nohash.to_string(),
                        size: 0
                    }, *(lib_path / l.name.to_path())));
                }
            }
        }
        Ok(())
    }
}
