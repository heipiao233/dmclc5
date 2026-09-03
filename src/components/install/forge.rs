//! [MinecraftForge](https://minecraftforge.net/) component installer.

#[cfg(feature = "mod_loaders")]
use crate::components::mods::ModLoader;
use crate::{LauncherConfig, components::{install::{ComponentInstaller, forgelike::ForgeLikeInstaller}, mods::{ModInfo, new_forgelike::NewerForgeLikeModLoader, old_forge::OldForgeModLoader}}, minecraft::schemas::{Argument, VersionJSON}};

use super::forgelike::ForgeLikeInstallerTrait;

/// A type mark for MinecraftForge installer.
#[derive(Clone, Copy)]
pub struct ForgeInstaller;
/// The MinecraftForge component type
pub const FORGE_INSTALLER: ComponentInstaller = ComponentInstaller::Forge(ForgeLikeInstaller(ForgeInstaller));

impl ForgeLikeInstallerTrait for ForgeInstaller {
    const SUPPORTS_OLDER_VERSION: bool = true;
    const MAVEN_GROUP_URL: &'static str = "https://maven.minecraftforge.net/net/minecraftforge";

    #[cfg(feature = "mod_loaders")]
    fn get_mod_loaders(version: &str, _: &LauncherConfig) -> Vec<ModLoader> {
        if version.split(".").nth(1).unwrap().parse::<usize>().unwrap() <= 13 {
            vec![OldForgeModLoader {
                version: version.split("-").nth(1).unwrap().to_string()
            }.into()]
        } else {
            let loader = NewerForgeLikeModLoader {
                builtin_mod: ModInfo {
                    name: Some("Forge".to_string()),
                    id: "forge".to_string(),
                    version: Some(versions::Versioning::new(version.split("-").nth(1).unwrap()).unwrap()),
                    desc: Some("Forge, a broad compatibility API.".to_string()),
                    license: "LGPL-2.1".to_string(),
                    depends: vec![],
                    recommends: vec![],
                    suggests: vec![],
                    conflicts: vec![],
                    breaks: vec![],
                },
                mods_toml_name: "mods.toml".to_string()
            };
            vec![loader.into()]
        }
    }

    fn find_in_version(mc: &VersionJSON) -> Option<String> {
        mc.get_base().libraries
            .iter()
            .find(|i|["fmlloader", "forge"].contains(&i.get_base().name.name.as_str()))
            .and_then(|i|i.get_base().name.version.clone().split("-").nth(1).map(|i|i.to_string()))
            .or_else(|| match mc {
                VersionJSON::New { arguments, base: _ } => {
                    arguments.game.as_ref()
                        .iter()
                        .flat_map(|i|i.windows(2))
                        .filter_map(|i| if Argument::String("--fml.forgeVersion".to_string()) == i[0] && let Argument::String(w) = &i[1] {
                            Some(w.clone())
                        } else { None })
                        .next()
                }
                _ => None
            })
    }

    fn get_archive_base_name(_mc_version: &str) -> String {
        "forge".to_string()
    }

    fn match_version(loader: &str, mc: &str) -> bool {
        loader.starts_with(&(mc.to_owned() + "-"))
    }
}
