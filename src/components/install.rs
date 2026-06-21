//! Things about installing components.

pub mod forgelike;
pub mod neoforge;
pub mod forge;

pub mod fabriclike;

use std::str::FromStr;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use enum_dispatch::enum_dispatch;
use tokio::sync::mpsc;

#[cfg(feature = "mod_loaders")]
use crate::components::mods::ModLoader;
use crate::{LauncherContext, components::install::{fabriclike::{FABRIC_INSTALLER, FabricInstaller, FabricLikeInstaller, QUILT_INSTALLER, QuiltInstaller}, forge::{FORGE_INSTALLER, ForgeInstaller}, forgelike::ForgeLikeInstaller, neoforge::{NEOFORGE_INSTALLER, NeoForgeInstaller}}, minecraft::{schemas::VersionJSON, version::{ComponentInfo, MinecraftInstallation}}, utils::DownloadAllMessage};

/// The interface for [ComponentInstaller].
#[async_trait]
#[enum_dispatch(ComponentInstaller)]
pub trait ComponentInstallerTrait: Send + Sync {
    /// Get suitable versions for a [MinecraftInstallation].
    async fn get_suitable_loader_versions(&self, mc: &MinecraftInstallation) -> Result<Vec<String>>;

    /// Install for a [MinecraftInstallation].
    /// Clients should not call this directly, as it doesn't append [crate::minecraft::version::DMCLCExtraData::components]
    /// Insteadly, clients should call [MinecraftInstallation::install_component].
    async fn install(&self, mc: &mut MinecraftInstallation, version: &str, download_channel: mpsc::UnboundedSender<DownloadAllMessage>) -> Result<()>;

    /// Find this component in a [MinecraftInstallation]. Returns the version of the component.
    fn find_in_version(&self, v: &VersionJSON) -> Option<String>;

    /// Get the mod loaders the component provides.
    /// For examples, the component quilt provides Quilt Loader and Fabric Loader, and the component OptiFine doesn't provide a mod loader;
    #[cfg(feature = "mod_loaders")]
    async fn get_mod_loaders(&self, version: &str, launcher: &LauncherContext) -> Result<Vec<ModLoader>>;
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

impl ToString for ComponentInstaller {
    fn to_string(&self) -> String {
        match self {
            ComponentInstaller::Fabric(_) => "fabric".to_string(),
            ComponentInstaller::Quilt(_) => "quilt".to_string(),
            ComponentInstaller::Forge(_) => "forge".to_string(),
            ComponentInstaller::NeoForge(_) => "neoforge".to_string(),
        }
    }
}

impl FromStr for ComponentInstaller {
    type Err = String;
    fn from_str(s: &str) -> std::prelude::v1::Result<Self, Self::Err> {
        match s {
            "fabric" => Ok(FABRIC_INSTALLER),
            "quilt" => Ok(QUILT_INSTALLER),
            "forge" => Ok(FORGE_INSTALLER),
            "neoforge" => Ok(NEOFORGE_INSTALLER),
            _ => Err(format!("Unknown loader {s}"))
        }
    }
}

impl MinecraftInstallation {
    /// Install a component.
    pub async fn install_component(&mut self, component: ComponentInstaller, version: &str, download_channel: mpsc::UnboundedSender<DownloadAllMessage>) -> Result<()> {
        if let None = self.extra_data.version {
            return Err(anyhow!(t!("loaders.minecraft_version_unknown")));
        }
        component.install(self, version, download_channel).await?;
        self.extra_data.components.push(ComponentInfo {
            name: component,
            version: version.to_string()
        });
        Ok(())
    }
}
