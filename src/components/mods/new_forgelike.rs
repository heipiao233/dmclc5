//! Implementation of [ModLoader] for Forge after 1.14 and NeoForge.

use std::{collections::HashMap, fs::File, io::{Cursor, Read, Seek}, path::Path};

use serde::Deserialize;
use versions::Versioning;
use zip::ZipArchive;
use crate::{components::mods::{ModsError, Result}, utils::deserialize_maven_version_range};

use super::{DepRequirement, ModInfo, ModLoaderTrait, VersionBound};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForgeNewMod {
    mod_id: String,
    #[serde(default="default_version")]
    version: String,
    display_name: Option<String>,
    description: Option<String>
}

fn default_version() -> String {
    "1".to_string()
}

#[derive(Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "UPPERCASE")]
enum Side {
    Both,
    Client,
    Server
}

#[derive(Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum DependencyType {
    Required,
    Optional,
    Discouraged,
    Incompatible,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Dependency {
    mod_id: String,
    r#type: Option<DependencyType>,
    #[serde(default)]
    mandatory: bool,
    reason: Option<String>,
    #[serde(deserialize_with = "deserialize_maven_version_range")]
    version_range: Vec<VersionBound>,
    side: Side
}

impl Into<DepRequirement> for Dependency {
    fn into(self) -> DepRequirement {
        DepRequirement {
            id: self.mod_id,
            version: self.version_range,
            reason: self.reason,
            unless: vec![]
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModsToml {
    license: String,
    mods: Vec<ForgeNewMod>,
    #[serde(default)]
    dependencies: HashMap<String, Vec<Dependency>>
}

/// The [ModLoader] implementation for Forge after 1.14 and NeoForge.
pub struct NewerForgeLikeModLoader {
    pub(in crate::components) builtin_mod: ModInfo,
    pub(in crate::components) mods_toml_name: String
}

#[derive(Deserialize)]
struct JIJEntry {
    path: String
}

#[derive(Deserialize)]
struct JIJInfo {
    jars: Vec<JIJEntry>
}

fn get_impl_version<R: Read + Seek>(for_mod: &str, file: &mut ZipArchive<R>) -> Result<Option<String>> {
    let mut manifest = file.by_name("META-INF/MANIFEST.MF")?;
    let mut manifest_content = String::new();
    manifest.read_to_string(&mut manifest_content)?;
    let line = manifest_content.lines()
        .filter_map(|l|l.strip_prefix("Implementation-Version:"))
        .map(ToString::to_string)
        .next();
    Ok(line)
}

impl NewerForgeLikeModLoader {
    fn get_mods_in_reader<R: Read + Seek>(&self, mut read: R) -> Result<Vec<ModInfo>> {
        let mut res = vec![];
        let mut jar_data = Vec::new();
        read.read_to_end(&mut jar_data)?;
        let mut stack: Vec<Cursor<Vec<u8>>> = vec![Cursor::new(jar_data)];

        while let Some(file) = stack.pop() {
            let mut archive = ZipArchive::new(file)?;
            let mut mod_toml = String::new();
            let mut impl_version = String::new();
            archive.by_name(&format!("META-INF/{}", self.mods_toml_name))?.read_to_string(&mut mod_toml)?;
            let mod_toml: ModsToml = toml::from_str(&mod_toml)?;
            for i in mod_toml.mods {
                let empty = vec![];
                let deps = mod_toml.dependencies.get(&i.mod_id).unwrap_or(&empty).iter().filter(|v|v.side != Side::Server);
                let depends = deps.clone()
                    .filter(|v|v.mandatory || v.r#type == Some(DependencyType::Required))
                    .map(Clone::clone).map(Into::into).collect::<Vec<_>>();
                let recommends = deps.clone()
                    .filter(|v|(v.r#type == None && !v.mandatory) || v.r#type == Some(DependencyType::Optional))
                    .map(Clone::clone).map(Into::into)
                    .collect::<Vec<_>>();

                let breaks = deps.clone()
                    .filter(|v|v.r#type == Some(DependencyType::Discouraged))
                    .map(Clone::clone).map(Into::into).collect::<Vec<_>>();
                let conflicts = deps.clone()
                    .filter(|v|v.r#type == Some(DependencyType::Discouraged))
                    .map(Clone::clone).map(Into::into)
                    .collect::<Vec<_>>();
                let mut version = i.version;
                if version == "${file.jarVersion}" {
                    if !impl_version.is_empty() {
                        version = impl_version.clone();
                    } else {
                        version = get_impl_version(&i.display_name.as_ref().unwrap_or(&i.mod_id), &mut archive)?
                            .unwrap_or("${file.jarVersion}".to_string());
                        impl_version = version.clone();
                    }
                }
                res.push(ModInfo {
                    name: i.display_name,
                    id: i.mod_id,
                    version: Some(Versioning::new(version).unwrap()),
                    desc: i.description,
                    license: mod_toml.license.clone(),
                    depends,
                    recommends,
                    suggests: vec![],
                    conflicts,
                    breaks
                });
            }
            let jij_jars = if let Ok(mut f) = archive.by_name("META-INF/jarjar/metadata.json") {
                let mut jij = String::new();
                f.read_to_string(&mut jij)?;
                let jij: JIJInfo = serde_json::from_str(&jij)?;
                jij.jars
            } else { vec![] };
            for i in jij_jars {
                if let Ok(mut jar_entry) = archive.by_name(&i.path) {
                    let mut jar_data = Vec::new();
                    jar_entry.read_to_end(&mut jar_data)?;
                    stack.push(Cursor::new(jar_data));
                }
            }
        }
        Ok(res)
    }
}

impl ModLoaderTrait for NewerForgeLikeModLoader {
    fn get_builtin_mods(&self) -> Vec<ModInfo> {
        vec![self.builtin_mod.clone()]
    }

    fn get_mods_in_file(&self, path: &Path) -> Result<Vec<ModInfo>> {
        self.get_mods_in_reader(File::open(path)?)
    }
}
