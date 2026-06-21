//! Things about mod and components.

#[cfg(feature="components_installation")]
use crate::components::install::{ComponentInstaller, fabriclike::{FABRIC_INSTALLER, QUILT_INSTALLER}, forge::FORGE_INSTALLER, neoforge::NEOFORGE_INSTALLER};

#[cfg(feature="components_installation")]
pub mod install;
#[cfg(feature="mod_loaders")]
pub mod mods;

#[cfg(feature="components_installation")]
/// All the supported components.
pub const COMPONENTS: [ComponentInstaller; 4] = [
    FORGE_INSTALLER,
    NEOFORGE_INSTALLER,
    FABRIC_INSTALLER,
    QUILT_INSTALLER
];
