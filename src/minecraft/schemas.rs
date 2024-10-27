#![doc(hidden)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::utils::maven_coord::ArtifactCoordinate;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LatestInfo {
    pub release: String,
    pub snapshot: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub typ: VersionType,
    pub url: String,
    pub time: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionList {
    pub latest: LatestInfo,
    pub versions: Vec<VersionInfo>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Resource {
    pub url: String,
    pub sha1: String,
    pub size: usize
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ResourceWithID {
    pub id: String,
    #[serde(flatten)]
    pub res: Resource
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct VersionJSONDownloads {
    pub client: Resource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_mappings: Option<Resource>,
    pub server: Resource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_mappings: Option<Resource>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct LoggingInfo {
    pub argument: String,
    pub file: ResourceWithID,
    #[serde(rename = "type")]
    pub typ: String
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct VersionJSONLogging {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<LoggingInfo>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Snapshot,
    #[default]
    Release,
    OldBeta,
    OldAlpha
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct JavaInfo {
    pub component: String,
    pub major_version: usize
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexInfo {
    pub total_size: usize,
    #[serde(flatten)]
    pub res: ResourceWithID
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LibraryArtifact {
    pub path: String,
    #[serde(flatten)]
    pub res: Resource
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LibraryBase {
    pub name: ArtifactCoordinate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<EnvRule>>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LibraryDownloadsVanillaNatives {
    pub classifiers: HashMap<String, LibraryArtifact>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LibraryDownloadsVanillaAndForge {
    pub artifact: LibraryArtifact
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LibraryVanillaNatives {
    pub downloads: LibraryDownloadsVanillaNatives,
    pub natives: HashMap<OSType, String>,
    #[serde(flatten)]
    pub base: LibraryBase
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LibraryVanillaForgeAndNeo {
    pub downloads: LibraryDownloadsVanillaAndForge,
    #[serde(flatten)]
    pub base: LibraryBase
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LibraryFabricOldForgeAndLiteLoader {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<Vec<String>>,
    #[serde(default = "returns_true")]
    pub clientreq: bool,
    #[serde(flatten)]
    pub base: LibraryBase
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LibraryFabricWithHash {
    pub url: String,
    #[serde(flatten)]
    pub base: LibraryBase,
    pub size: usize,
    pub sha1: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Library {
    VanillaNatives(LibraryVanillaNatives),
    VanillaForgeAndNeo(LibraryVanillaForgeAndNeo),
    FabricWithHash(LibraryFabricWithHash),
    FabricOldForgeAndLiteLoader(LibraryFabricOldForgeAndLiteLoader),
    BaseOnly(LibraryBase)
}

impl Library {
    pub fn get_base(&self) -> &LibraryBase {
        match self {
            Library::FabricWithHash(l) => &l.base,
            Library::FabricOldForgeAndLiteLoader(l) => &l.base,
            Library::VanillaForgeAndNeo(l) => &l.base,
            Library::VanillaNatives(l) => &l.base,
            Library::BaseOnly(l) => l,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionJSONBase {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub asset_index: AssetIndexInfo,
    #[serde(default)]
    pub assets: String,
    #[serde(default)]
    pub compliance_level: isize,
    #[serde(default)]
    pub downloads: VersionJSONDownloads,

    pub id: String,
    #[serde(default)]
    pub java_version: JavaInfo,
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub logging: VersionJSONLogging,

    #[serde(default)]
    pub main_class: String,
    #[serde(default)]
    pub minimum_launcher_version: usize,
    #[serde(default)]
    pub release_time: String,
    #[serde(default)]
    pub time: String,
    #[serde(default)]
    #[serde(rename = "type")]
    pub typ: VersionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum OSType {
    Linux,
    Windows,
    #[serde(rename = "osx")]
    OSX
    // Other OSes are not supported by Mojang.
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RulePlatform {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<OSType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum EnvRuleType {
    Allow,
    Disallow
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnvRule {
    pub action: EnvRuleType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<HashMap<String, bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<RulePlatform>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum OneOrMoreArguments {
    One(String),
    More(Vec<String>)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Argument {
    String(String),
    Conditional {
        rules: Vec<EnvRule>,
        value: OneOrMoreArguments
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Arguments {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game: Option<Vec<Argument>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jvm: Option<Vec<Argument>>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum VersionJSON {
    Old {
        #[serde(rename = "minecraftArguments")]
        minecraft_arguments: String,
        #[serde(flatten)]
        base: VersionJSONBase
    },
    New {
        arguments: Arguments,
        #[serde(flatten)]
        base: VersionJSONBase
    }
}

impl VersionJSON {
    pub fn get_base(&self) -> &VersionJSONBase {
        match self {
            Self::Old { base, .. } => base,
            Self::New { base, .. } => base
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Asset {
    pub hash: String,
    pub size: usize
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AssetsIndex {
    pub objects: HashMap<String, Asset>
}

fn returns_true() -> bool {
    true
}
