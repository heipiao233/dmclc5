//! Implementation of [ModLoader] for Quilt Loader.

use std::{collections::HashMap, fs::File, io::{Read, Seek}};

use acc_reader::AccReader;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use versions::Versioning;
use zip::ZipArchive;

use crate::utils::{json_newline_transform, BetterPath};

use super::{fabric::{fabric_parse_req, Icons}, DepRequirement, ModInfo, ModLoader, VersionBound};

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum Env {
    #[serde(rename = "*")]
    #[default]
    All,
    Client,
    DedicatedServer
}

#[derive(Deserialize, Serialize)]
struct ProvidesObject {
    id: String,
    version: String
}

#[derive(Deserialize, Serialize, Clone)]
struct DependencyObject {
    id: String,
    #[serde(default)]
    version: Vec<String>,
    #[serde(default)]
    optional: bool,
    reason: Option<String>,
    #[serde(default)]
    unless: Vec<DependencyObject>
}

impl Into<DepRequirement> for DependencyObject {
    fn into(self) -> DepRequirement {
        DepRequirement {
            id: self.id,
            version: self.version.into_iter().map(|v|fabric_parse_req(&v)).map(VersionBound::new_one).collect(),
            reason: self.reason,
            unless: self.unless.into_iter().map(Into::into).collect()
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum Licenses {
    SingleString(String),
    MultiString(Vec<String>),
    Object {
        name: String,
        id: String,
        url: String,
        #[serde(default)]
        description: String
    }
}

impl ToString for Licenses {
    fn to_string(&self) -> String {
        match self {
            Licenses::SingleString(v) => v.clone(),
            Licenses::MultiString(v) => v.join(", "),
            Licenses::Object { name, id: _, url: _, description: _ } => name.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct QuiltModJsonMetadataMinecraft {
    #[serde(default)]
    environment: Env
}

#[derive(Deserialize, Serialize, Default)]
struct QuiltModJsonMetadata {
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    contributors: HashMap<String, String>,
    #[serde(default)]
    contact: HashMap<String, String>,
    #[serde(default)]
    license: Option<Licenses>,
    #[serde(default)]
    icon: Option<Icons>,
    #[serde(default)]
    minecraft: QuiltModJsonMetadataMinecraft
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum Dependency {
    One(DependencyObject),
    Many(Vec<DependencyObject>),
    OneString(String)
}

impl Default for Dependency {
    fn default() -> Self {
        Self::Many(vec![])
    }
}

impl Into<Vec<DependencyObject>> for Dependency {
    fn into(self) -> Vec<DependencyObject> {
        match self {
            Dependency::One(v) => vec![v],
            Dependency::Many(v) => v,
            Dependency::OneString(v) => vec![DependencyObject {
                id: v,
                version: vec![],
                optional: false,
                reason: None,
                unless: vec![]
            }],
        }
    }
}

#[derive(Deserialize, Serialize)]
struct QuiltModJsonInner {
    group: String,
    id: String,
    #[serde(default)]
    provides: Vec<ProvidesObject>,
    version: String,
    #[serde(default)]
    jars: Vec<String>,
    #[serde(default)]
    depends: Dependency,
    #[serde(default)]
    breaks: Dependency,
    #[serde(default)]
    load_type: String,
    #[serde(default)]
    metadata: QuiltModJsonMetadata
}


#[derive(Serialize, Deserialize)]
struct QuiltModJson {
    quilt_loader: QuiltModJsonInner
}

/// The [ModLoader] implementation for Quilt Loader.
pub struct QuiltModLoader {
    pub(in crate::components) builtin_mods: Option<Vec<ModInfo>>
}

impl QuiltModLoader {
    fn get_mods_in_reader<R: Read + Seek>(&self, read: R) -> Result<Vec<ModInfo>> {
        let mut res = vec![];
        let mut archive = ZipArchive::new(read)?;
        let mut mod_json = String::new();
        archive.by_name("quilt.mod.json")?.read_to_string(&mut mod_json)?;
        let mod_json: QuiltModJsonInner = serde_json::from_str::<QuiltModJson>(&json_newline_transform(&mod_json))?.quilt_loader;
        let depends = Into::<Vec<DependencyObject>>::into(mod_json.depends.clone()).iter()
            .filter(|v|!v.optional)
            .map(Clone::clone).map(Into::into).collect::<Vec<_>>();
        let recommends = Into::<Vec<DependencyObject>>::into(mod_json.depends).iter()
            .filter(|v|v.optional)
            .map(Clone::clone).map(Into::into)
            .collect::<Vec<_>>();

        let breaks = Into::<Vec<DependencyObject>>::into(mod_json.breaks.clone()).iter()
            .filter(|v|!v.optional)
            .map(Clone::clone).map(Into::into).collect::<Vec<_>>();
        let conflicts = Into::<Vec<DependencyObject>>::into(mod_json.breaks).iter()
            .filter(|v|v.optional)
            .map(Clone::clone).map(Into::into)
            .collect::<Vec<_>>();
        res.push(ModInfo {
            name: mod_json.metadata.name,
            id: mod_json.id,
            version: Some(Versioning::new(mod_json.version).unwrap()),
            desc: mod_json.metadata.description,
            license: mod_json.metadata.license.unwrap_or(Licenses::SingleString("All Rights Reserved".to_string())).to_string(),
            depends,
            recommends,
            suggests: vec![],
            conflicts,
            breaks,
        });
        for i in mod_json.provides {
            res.push(ModInfo {
                name: None,
                id: i.id.clone(),
                version: Some(Versioning::new(i.version).unwrap()),
                desc: None,
                license: res[0].license.clone(),
                depends: vec![],
                recommends: vec![],
                suggests: vec![],
                conflicts: vec![],
                breaks: vec![]
            });
        }
        for i in mod_json.jars {
            res.append(&mut self.get_mods_in_reader(AccReader::new(archive.by_name(&i)?))?);
        }
        Ok(res)
    }
}

impl ModLoader for QuiltModLoader {
    fn get_builtin_mods(&self) -> Vec<ModInfo> {
        self.builtin_mods.clone().into_iter().flatten().collect()
    }

    fn get_mods_in_file(&self, path: &BetterPath) -> Result<Vec<ModInfo>> {
        self.get_mods_in_reader(File::open(path)?)
    }
}
