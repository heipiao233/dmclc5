//! Things about installing components.

pub mod forgelike;
pub(crate) mod neoforge;
pub(crate) mod forge;

pub mod fabriclike;

use anyhow::{anyhow, Result};
use async_trait::async_trait;

use crate::{minecraft::{schemas::VersionJSON, version::{ComponentInfo, MinecraftInstallation}}, LauncherContext};

use super::mods::ModLoader;

/// A installer for a component.
/// A "component" is something like Forge, NeoForge, Fabric, Quilt, LiteLoader and OptiFine.
#[async_trait]
pub trait ComponentInstaller: Send + Sync {
    /// Get suitable versions for a [MinecraftInstallation].
    async fn get_suitable_loader_versions(&self, mc: &MinecraftInstallation) -> Result<Vec<String>>;

    /// Install for a [MinecraftInstallation].
    /// Clients should not call this directly, as it doesn't append [crate::minecraft::version::DMCLCExtraData::components]
    /// Insteadly, clients should call [MinecraftInstallation::install_component].
    async fn install(&self, mc: &mut MinecraftInstallation, version: &str) -> Result<()>;

    /// Find this component in a [MinecraftInstallation]. Returns the version of the component.
    fn find_in_version(&self, v: &VersionJSON) -> Option<String>;

    /// Get the mod loaders the component provides.
    /// For examples, the component quilt provides Quilt Loader and Fabric Loader, and the component OptiFine doesn't provide a mod loader;
    #[cfg(feature = "mod_loaders")]
    async fn get_mod_loaders(&self, version: &str, launcher: &LauncherContext) -> Result<Vec<Box<dyn ModLoader>>>;
}

impl MinecraftInstallation<'_> {
    /// Install a component.
    pub async fn install_component(&mut self, component: &str, version: &str) -> Result<()> {
        if let None = self.extra_data.version {
            return Err(anyhow!(t!("loaders.minecraft_version_unknown")));
        }
        self.launcher.component_installers[component].install(self, version).await?;
        self.extra_data.components.push(ComponentInfo {
            name: component.to_string(),
            version: version.to_string()
        });
        Ok(())
    }
}
