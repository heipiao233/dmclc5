#[cfg(feature = "mod_loaders")]
use crate::components::mods::ModLoader;
use crate::{LauncherContext, components::{install::{ComponentInstaller, forgelike::ForgeLikeInstaller}, mods::{ModInfo, ModLoaderTrait, new_forgelike::NewerForgeLikeModLoader, old_forge::OldForgeModLoader}}, minecraft::schemas::{Argument, VersionJSON}};

use super::forgelike::ForgeLikeInstallerTrait;

#[derive(Clone, Copy)]
pub struct ForgeInstaller;
pub const FORGE_INSTALLER: ComponentInstaller = ComponentInstaller::Forge(ForgeLikeInstaller(ForgeInstaller));

impl ForgeLikeInstallerTrait for ForgeInstaller {
    const SUPPORTS_OLDER_VERSION: bool = true;
    const MAVEN_GROUP_URL: &'static str = "https://maven.minecraftforge.net/net/minecraftforge";

    #[cfg(feature = "mod_loaders")]
    fn get_mod_loaders(version: &str, _: &LauncherContext) -> Vec<ModLoader> {
        if version.split(".").collect::<Vec<&str>>()[1].parse::<usize>().unwrap() <= 13 {
            vec![OldForgeModLoader {
                version: version.split("-").collect::<Vec<_>>()[1].to_string()
            }.into()]
        } else {
            let loader = NewerForgeLikeModLoader {
                builtin_mod: ModInfo {
                    name: Some("Forge".to_string()),
                    id: "forge".to_string(),
                    version: Some(versions::Versioning::new(version.split("-").collect::<Vec<&str>>()[1]).unwrap()),
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
        for l in &mc.get_base().libraries {
            let coord = &l.get_base().name;
            if ["fmlloader", "forge"].contains(&coord.name.as_str()) {
                return Some(coord.version.clone().split("-").collect::<Vec<&str>>()[1].to_string());
            }
        }

        if let VersionJSON::New { arguments, base: _ } = mc {
            for arg2 in arguments.game.as_ref()?.windows(2) {
                if let Argument::String(v) = &arg2[0] && v == "--fml.forgeVersion" && let Argument::String(w) = &arg2[1] {
                    return Some(w.to_string());
                }
            }
        }
        None
    }

    fn get_archive_base_name(_mc_version: &str) -> String {
        "forge".to_string()
    }

    fn match_version(loader: &str, mc: &str) -> bool {
        loader.starts_with(&(mc.to_owned() + "-"))
    }
}
