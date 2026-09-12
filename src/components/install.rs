//! Things about installing components.

pub mod forgelike;
pub mod neoforge;
pub mod forge;

pub mod fabriclike;

use std::str::FromStr;

use enum_dispatch::enum_dispatch;
use tokio::sync::mpsc;

#[cfg(feature = "mod_loaders")]
use crate::components::mods::{ModLoader, ModsError};
use crate::{LauncherConfig, components::install::{fabriclike::{FABRIC_INSTALLER, FabricInstaller, FabricLikeInstaller, QUILT_INSTALLER, QuiltInstaller}, forge::{FORGE_INSTALLER, ForgeInstaller}, forgelike::ForgeLikeInstaller, neoforge::{NEOFORGE_INSTALLER, NeoForgeInstaller}}, minecraft::{schemas::VersionJSON, version::{ComponentInfo, MinecraftInstallation}}, utils::download::DownloadAllMessage};

/// The interface for [ComponentInstaller].
#[allow(async_fn_in_trait)]
#[enum_dispatch(ComponentInstaller)]
pub trait ComponentInstallerTrait {
    /// Get suitable versions for a [MinecraftInstallation].
    async fn get_suitable_loader_versions(&self, mc: &MinecraftInstallation<'_>) -> Result<Vec<String>>;

    /// Install for a [MinecraftInstallation].
    /// Clients should not call this directly, as it doesn't append [crate::minecraft::version::DMCLCExtraData::components]
    /// Insteadly, clients should call [MinecraftInstallation::install_component].
    async fn install(&self, mc: &mut MinecraftInstallation<'_>, version: &str, download_channel: mpsc::Sender<DownloadAllMessage>) -> Result<()>;

    /// Find this component in a [MinecraftInstallation]. Returns the version of the component.
    fn find_in_version(&self, v: &VersionJSON) -> Option<String>;

    /// Get the mod loaders the component provides.
    /// For examples, the component quilt provides Quilt Loader and Fabric Loader, and the component OptiFine doesn't provide a mod loader;
    #[cfg(feature = "mod_loaders")]
    async fn get_mod_loaders(&self, version: &str, launcher: &LauncherConfig) -> std::result::Result<Vec<ModLoader>, ModsError>;
}

/// A installer for a component.
/// A "component" is a something needing, like Forge, NeoForge, Fabric, Quilt, LiteLoader and OptiFine.
#[enum_dispatch]
#[allow(missing_docs)]
#[derive(Clone, Copy)]
pub enum ComponentInstaller {
    Forge(ForgeLikeInstaller<ForgeInstaller>),
    NeoForge(ForgeLikeInstaller<NeoForgeInstaller>),
    Fabric(FabricLikeInstaller<FabricInstaller>),
    Quilt(FabricLikeInstaller<QuiltInstaller>)
}

impl ComponentInstaller {
    /// Get the name of the installer
    pub fn as_str(&self) -> &str {
        match self {
            ComponentInstaller::Fabric(_) => "fabric",
            ComponentInstaller::Quilt(_) => "quilt",
            ComponentInstaller::Forge(_) => "forge",
            ComponentInstaller::NeoForge(_) => "neoforge",
        }
    }
}

impl ToString for ComponentInstaller {
    fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

impl FromStr for ComponentInstaller {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "fabric" => Ok(FABRIC_INSTALLER),
            "quilt" => Ok(QUILT_INSTALLER),
            "forge" => Ok(FORGE_INSTALLER),
            "neoforge" => Ok(NEOFORGE_INSTALLER),
            _ => Err(format!("Unknown loader {s}"))
        }
    }
}

/// Errors from component installers.
#[derive(thiserror::Error, Debug)]
pub enum ComponentInstallerError {
    /// Failure when reading a ZIP file.
    #[error("Zip File Error: {0}")]
    ZipError(#[from] zip::result::ZipError),
    /// Failure when downloading.
    #[error("Download Error: {0}")]
    DownloadError(#[from] crate::utils::download::DownloadError),
    /// Failure when (de)serializing JSON.
    #[error("JSON Serialize/Deserialize Error: {0}")]
    JsonError(#[from] serde_json::Error),
    /// Failure when deserializing XML.
    #[error("XML Read Error: {0}")]
    XmlError(#[from] xmltree::ParseError),
    /// Network error.
    #[error("Network Error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    /// IO error.
    #[error("IO Error: {0}")]
    IOError(#[from] std::io::Error),
    /// FS error from [fs_extra]
    #[error("FS (extra) Error: {0}")]
    FSExtError(#[from] fs_extra::error::Error),
    /// Two different kinds of `version.json` are asked to be merged.
    #[error("Cannot merge `version.json`s.")]
    VersionJSONMergeError,
    /// A Forge installer processor fails.
    #[error("A processor failed to run: {0}")]
    ProcessorFailureError(String),
    /// A Forge installer processor has no main class.
    #[error("No main class in processor jar")]
    ProcessorNotExecutableError,
    /// Minecraft version is unknown.
    #[error("Minecraft version unknown")]
    MinecraftVersionUnknown,
}

pub(self) type Result<T> = std::result::Result<T, ComponentInstallerError>;

impl MinecraftInstallation<'_> {
    /// Install a component.
    pub async fn install_component(&mut self, component: ComponentInstaller, version: &str, download_channel: mpsc::Sender<DownloadAllMessage>) -> Result<()> {
        if let None = self.extra_data.version {
            return Err(ComponentInstallerError::MinecraftVersionUnknown);
        }
        component.install(self, version, download_channel).await?;
        self.extra_data.components.push(ComponentInfo {
            name: component,
            version: version.to_string()
        });
        Ok(())
    }
}
