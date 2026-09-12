//! Implementation of [ComponentInstaller] for Fabric-like installers.

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[cfg(feature = "mod_loaders")]
use crate::components::mods::ModsError;
use crate::{LauncherConfig, components::{install::{ComponentInstaller, ComponentInstallerError, Result}, mods::{ModLoader, ModLoaderTrait, fabric::FabricModLoader, quilt::QuiltModLoader}}, minecraft::{schemas::VersionJSON, version::MinecraftInstallation}, utils::{download::{DownloadAllMessage, download, download_all}, maven_coord::ArtifactCoordinate, merge_version_json}};

use super::ComponentInstallerTrait;

/// A [ComponentInstaller] implementation for Fabric-like components.
/// We don't install Fabric API or QSL.
pub trait FabricLikeInstallerTrait {
    /// Metadata API URL
    const META_URL: &'static str;
    /// Maven artifact name of the loader jar, which can indicate the version.
    const LOADER_ARTIFACT_NAME: &'static str;
    #[cfg(feature = "mod_loaders")]
    #[allow(async_fn_in_trait)]
    /// Get the mod loaders the component provides.
    async fn get_loader(version: &str, launcher: &LauncherConfig) -> std::result::Result<Vec<ModLoader>, ModsError>;
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

/// A type mark for Fabric Loader installer.
#[derive(Clone, Copy)]
pub struct FabricInstaller;

impl FabricLikeInstallerTrait for FabricInstaller {
    const META_URL: &'static str = "https://meta.fabricmc.net/v2";
    const LOADER_ARTIFACT_NAME: &'static str = "fabric-loader";

    #[cfg(feature = "mod_loaders")]
    async fn get_loader(version: &str, launcher: &LauncherConfig) -> std::result::Result<Vec<ModLoader>, ModsError> {
        let mut loader = FabricModLoader {
            builtin_mods: None
        };
        let filepath = format!("net/fabricmc/fabric-loader/{version}/fabric-loader-{version}.jar");
        let path = launcher.get_libraries_path(&filepath);
        if !path.exists() {
            download(format!("https://maven.fabricmc.net/{filepath}"), &path).await?;
        }
        let path = launcher.get_libraries_path(&format!("net/fabricmc/fabric-loader/{version}/fabric-loader-{version}.jar"));
        loader.builtin_mods = Some(loader.get_mods_in_file(&path).ok().into_iter().flatten().collect());
        Ok(vec![loader.into()])
    }
}

/// A type mark for Quilt Loader installer.
#[derive(Clone, Copy)]
pub struct QuiltInstaller;

impl FabricLikeInstallerTrait for QuiltInstaller {
    const META_URL: &'static str = "https://meta.quiltmc.org/v3";
    const LOADER_ARTIFACT_NAME: &'static str = "quilt-loader";

    #[cfg(feature = "mod_loaders")]
    async fn get_loader(version: &str, launcher: &LauncherConfig) -> std::result::Result<Vec<ModLoader>, ModsError> {
        let mut loader = QuiltModLoader {
            builtin_mods: None
        };
        let filepath = format!("org/quiltmc/quilt-loader/{version}/quilt-loader-{version}.jar");
        let path = launcher.get_libraries_path(&filepath);
        if !path.exists() {
            download(format!("https://maven.quiltmc.org/repository/release/{filepath}"), &path).await?;
        }
        loader.builtin_mods = Some(loader.get_mods_in_file(&path).ok().into_iter().flatten().collect());

        let quilt_loader = QuiltModLoader {
            builtin_mods: None
        };
        Ok(vec![loader.into(), quilt_loader.into()])
    }
}

/// A type mark for Fabric-like installers.
#[derive(Clone, Copy)]
pub struct FabricLikeInstaller<T: FabricLikeInstallerTrait>(T);
/// The Fabric Loader component type
pub static FABRIC_INSTALLER: ComponentInstaller = ComponentInstaller::Fabric(FabricLikeInstaller(FabricInstaller));
/// The Quilt Loader component type
pub static QUILT_INSTALLER: ComponentInstaller = ComponentInstaller::Quilt(FabricLikeInstaller(QuiltInstaller));

impl <T: FabricLikeInstallerTrait> ComponentInstallerTrait for FabricLikeInstaller<T> {
    #[cfg(feature = "mod_loaders")]
    async fn get_mod_loaders(&self, version: &str, launcher: &LauncherConfig) -> std::result::Result<Vec<ModLoader>, ModsError> {
        T::get_loader(version, launcher).await
    }

    async fn get_suitable_loader_versions(&self, mc: &MinecraftInstallation<'_>) -> Result<Vec<String>> {
        let mcversion = mc.extra_data.version.as_ref().unwrap();
        let versions: Vec<FabricLikeVersionInfo> = reqwest::get(
            format!("{}/versions/loader/{}", T::META_URL, form_urlencoded::byte_serialize(mcversion.as_bytes()).collect::<String>())
        ).await?.json().await?;
        let res = versions.iter().map(|v|v.loader.version.clone()).collect();
        Ok(res)
    }

    async fn install(&self, mc: &mut MinecraftInstallation<'_>, version: &str, download_channel: mpsc::Sender<DownloadAllMessage>) -> Result<()> {
        let mcversion = mc.extra_data.version.as_ref().unwrap();
        let version_info: VersionJSON = reqwest::get(format!("{}/versions/loader/{}/{}/profile/json", T::META_URL,
            form_urlencoded::byte_serialize(mcversion.as_bytes()).collect::<String>(),
            form_urlencoded::byte_serialize(version.as_bytes()).collect::<String>())
        ).await?.json().await?;
        mc.obj = merge_version_json(&mc.obj, &version_info).map_err(|_| ComponentInstallerError::VersionJSONMergeError)?;
        serde_json::to_writer(&std::fs::File::create(mc.version_root.join(mc.name.to_string() + ".json"))?, &mc.obj)?;
        let res = mc.libraries(&version_info.get_base().libraries, true);
        download_all(res, download_channel,
            mc.config.download_parallel_files, mc.config.download_retries,
            mc.config.bmclapi_mirror.clone()
        ).await;
        Ok(())
    }

    fn find_in_version(&self, v: &VersionJSON) -> Option<String> {
        v.get_base()
            .libraries.iter()
            .find(|i|i.get_base().name.name == T::LOADER_ARTIFACT_NAME)
            .map(|i|i.get_base().name.version.clone())
    }
}
