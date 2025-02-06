//! Implementation of [ComponentInstaller] for Fabric-like installers.

use std::{future::Future, pin::Pin};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{components::mods::{fabric::FabricModLoader, quilt::QuiltModLoader, ModLoader}, minecraft::{schemas::VersionJSON, version::MinecraftInstallation}, utils::{download, download_all, maven_coord::ArtifactCoordinate, merge_version_json, DownloadAllMessage}, LauncherContext};

use super::ComponentInstaller;

#[cfg(feature = "mod_loaders")]
type GetLoader = fn(String, &'_ LauncherContext) -> Pin<Box<dyn
    Future<Output = Result<Vec<Box<dyn ModLoader>>>> + Send + '_
>>;

/// A [ComponentInstaller] implementation for Fabric-like components.
/// We don't install Fabric API or QSL.
pub struct FabricLikeInstaller {
    meta_url: String,
    loader_artifact_name: String,
    #[cfg(feature = "mod_loaders")]
    get_loader: GetLoader,
}

#[derive(Serialize, Deserialize, Clone)]
struct FabricLikeVersionInfo {
    loader: Version
}

#[derive(Serialize, Deserialize, Clone)]
struct Version {
    maven: ArtifactCoordinate,
    version: String
}

#[cfg(feature = "mod_loaders")]
async fn fabric_get_loader(version: &str, launcher: &LauncherContext) -> Result<Vec<Box<dyn ModLoader>>> {
    let mut loader = FabricModLoader {
        builtin_mods: None
    };
    let filepath = format!("net/fabricmc/fabric-loader/{version}/fabric-loader-{version}.jar");
    let path = &launcher.root_path / "libraries" / &filepath;
    if !path.0.exists() {
        download(format!("https://maven.fabricmc.net/{filepath}"), &path).await?;
    }
    let path = &launcher.root_path / "libraries/net/fabricmc/fabric-loader" / version / format!("fabric-loader-{version}.jar");
    loader.builtin_mods = Some(loader.get_mods_in_file(&path).ok().into_iter().flatten().collect());
    Ok(vec![Box::new(loader)])
}

#[cfg(feature = "mod_loaders")]
fn fabric_get_loader_boxpin(version: String, launcher: &'_ LauncherContext) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn ModLoader>>>> + Send + '_>> {
    Box::pin(async move {
        fabric_get_loader(&version, &launcher).await
    })
}

#[cfg(feature = "mod_loaders")]
async fn quilt_get_loader(version: &str, launcher: &LauncherContext) -> Result<Vec<Box<dyn ModLoader>>> {
    let mut loader = QuiltModLoader {
        builtin_mods: None
    };
    let filepath = format!("org/quiltmc/quilt-loader/{version}/quilt-loader-{version}.jar");
    let path = &launcher.root_path / "libraries" / &filepath;
    if !path.0.exists() {
        download(format!("https://maven.quiltmc.org/repository/release/{filepath}"), &path).await?;
    }
    loader.builtin_mods = Some(loader.get_mods_in_file(&path).ok().into_iter().flatten().collect());

    let fabric_loader = FabricModLoader {
        builtin_mods: None
    };
    Ok(vec![Box::new(loader), Box::new(fabric_loader)])
}

#[cfg(feature = "mod_loaders")]
fn quilt_get_loader_boxpin(version: String, launcher: &'_ LauncherContext) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn ModLoader>>>> + Send + '_>> {
    Box::pin(async move {
        quilt_get_loader(&version, &launcher).await
    })
}

impl FabricLikeInstaller {
    /// Return the [ComponentInstaller] for Fabric Loader.
    pub fn fabric() -> Self {
        FabricLikeInstaller {
            meta_url: "https://meta.fabricmc.net/v2".to_string(),
            loader_artifact_name: "fabric-loader".to_string(),
            #[cfg(feature = "mod_loaders")]
            get_loader: fabric_get_loader_boxpin
        }
    }

    /// Return the [ComponentInstaller] for Quilt Loader.
    pub fn quilt() -> Self {
        FabricLikeInstaller {
            meta_url: "https://meta.quiltmc.org/v3".to_string(),
            loader_artifact_name: "quilt-loader".to_string(),
            #[cfg(feature = "mod_loaders")]
            get_loader: quilt_get_loader_boxpin
        }
    }
}

#[async_trait]
impl ComponentInstaller for FabricLikeInstaller {
    #[cfg(feature = "mod_loaders")]
    async fn get_mod_loaders(&self, version: &str, launcher: &LauncherContext) -> Result<Vec<Box<dyn ModLoader>>> {
        (self.get_loader)(version.to_string(), launcher).await
    }

    async fn get_suitable_loader_versions(&self, mc: &MinecraftInstallation) -> Result<Vec<String>> {
        let mcversion = mc.extra_data.version.as_ref().unwrap();
        let versions: Vec<FabricLikeVersionInfo> = reqwest::get(
            format!("{}/versions/loader/{}", self.meta_url, form_urlencoded::byte_serialize(mcversion.as_bytes()).collect::<String>())
        ).await?.json().await?;
        let res = versions.iter().map(|v|v.loader.version.clone()).collect();
        Ok(res)
    }

    async fn install(&self, mc: &mut MinecraftInstallation, version: &str, download_channel: mpsc::UnboundedSender<DownloadAllMessage>) -> Result<()> {
        let mcversion = mc.extra_data.version.as_ref().unwrap();
        let version_info: VersionJSON = reqwest::get(format!("{}/versions/loader/{}/{}/profile/json", self.meta_url,
            form_urlencoded::byte_serialize(mcversion.as_bytes()).collect::<String>(),
            form_urlencoded::byte_serialize(version.as_bytes()).collect::<String>())
        ).await?.json().await?;
        mc.obj = merge_version_json(&mc.obj, &version_info)?;
        serde_json::to_writer(&std::fs::File::create(&mc.version_root / (mc.name.to_string() + ".json"))?, &mc.obj)?;
        let res = mc.install_libraries(&version_info.get_base().libraries, true)?;
        download_all(&res, download_channel).await?;
        Ok(())
    }

    fn find_in_version(&self, v: &VersionJSON) -> Option<String> {
        for i in &v.get_base().libraries {
            if i.get_base().name.name == self.loader_artifact_name {
                return Some(i.get_base().name.version.clone());
            }
        }
        None
    }
}
