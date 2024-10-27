use std::{collections::HashMap, fmt::Debug, sync::LazyLock};

use anyhow::Result;
use async_trait::async_trait;
use murmur2::murmur2;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use serde_repr::{Deserialize_repr, Serialize_repr};
use tokio::{fs::File, io::AsyncReadExt};

use crate::{minecraft::version::MinecraftInstallation, utils::BetterPath, LauncherContext};

use super::{Content, ContentDependency, ContentService, ContentType, ContentVersion, Screenshot};
#[cfg(feature="mod_loaders")]
const LOADER_TO_CURSEFORGE: LazyLock<HashMap<String, usize>> = LazyLock::new(||HashMap::from([
    ("forge".to_string(), 1),
    ("fabric".to_string(), 4),
    ("quilt".to_string(), 5),
    ("neoforge".to_string(), 6)
]));

const CONTENT_TYPE_TO_CURSEFORGE: LazyLock<HashMap<ContentType, &str>> = LazyLock::new(||HashMap::from([
    (ContentType::ModPack, "4471"),
    (ContentType::Shader, "4546"),
    (ContentType::Mod, "6"),
    (ContentType::ResourcePack, "12"),
    (ContentType::World, "17"),
]));

const API_KEY: &str = "$2a$10$VhDVvjRWDxOlbRnuqi1GEOCxcZ.fZGRLf2kg7pdN8i4dowykR4huy";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModLinks {
    website_url: Option<String>,
    wiki_url: Option<String>,
    issues_url: Option<String>,
    source_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Author {
    id: usize,
    name: String,
    url: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Category {
    id: usize,
    game_id: usize,
    name: String,
    slug: String,
    url: String,
    icon_url: String,
    date_modified: String,
    #[serde(default)]
    is_class: bool,
    #[serde(default)]
    class_id: usize,
    #[serde(default)]
    parent_category_id: usize,
    #[serde(default)]
    display_index: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModAsset {
    id: usize,
    mod_id: usize,
    title: String,
    description: String,
    thumbnail_url: String,
    url: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeMod {
    id: usize,
    game_id: usize,
    name: String,
    slug: String,
    links: ModLinks,
    summary: String,
    download_count: usize,
    is_featured: bool,
    primary_category_id: usize,
    categories: Vec<Category>,
    #[serde(default)]
    class_id: usize,
    authors: Vec<Author>,
    logo: ModAsset,
    screenshots: Vec<ModAsset>,
    main_file_id: usize,
    date_created: String,
    date_modified: String,
    date_released: String,
    #[serde(default)]
    allow_mod_distribution: bool,
    game_popularity_rank: usize,
    is_available: bool,
}

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Eq)]
#[repr(u8)]
enum Algorithm {
    SHA1 = 1,
    MD5 = 2,
}

#[derive(Serialize_repr, Deserialize_repr)]
#[repr(u8)]
enum ReleaseType {
    RELEASE = 1,
    BETA = 2,
    ALPHA = 3,
}

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Eq)]
#[repr(u8)]
enum RelationType {
    EmbeddedLibrary = 1,
    OptionalDependency = 2,
    RequiredDependency = 3,
    Tool = 4,
    Incompatible = 5,
    Include = 6,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeModFile {
    id: usize,
    mod_id: usize,
    is_available: bool,
    display_name: String,
    file_name: String,
    release_type: ReleaseType,
    hashes: Vec<Hash>,
    file_date: String,
    file_length: i64,
    download_count: usize,
    download_url: Option<String>,
    dependencies: Vec<Dependency>,
    #[serde(default)]
    is_server_pack: bool,
}



#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Hash {
    value: String,
    algo: Algorithm,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Dependency {
    mod_id: usize,
    relation_type: RelationType,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DataWrapped<T> {
    data: T
}

pub(crate) struct CurseforgeContentService;

impl Debug for CurseforgeMod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CurseforgeMod({})", self.name)
    }
}

impl Debug for CurseforgeModFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CurseforgeModFile({})", self.file_name)
    }
}

impl CurseforgeMod {
    async fn from_id(id: &str, launcher: &LauncherContext) -> Result<Self> {
        Ok(launcher.http_client.get(format!("https://api.curseforge.com/v1/mods/{id}")).header("x-api-key", API_KEY).send().await?.json::<DataWrapped<_>>().await?.data)
    }
}

impl CurseforgeModFile {
    async fn from_id(modid: &str, id: &str, launcher: &LauncherContext) -> Result<Self> {
        Ok(launcher.http_client.get(format!("https://api.curseforge.com/v1/mods/{modid}/files/{id}")).header("x-api-key", API_KEY).send().await?.json::<DataWrapped<_>>().await?.data)
    }
}

#[async_trait]
impl Content for CurseforgeMod {
    /**
     * List versions.
     * @param forVersion The Minecraft version you download for.
     * @throws RequestError
     */
    async fn list_downloadable_versions(&self, for_version: Option<&MinecraftInstallation<'_>>, launcher: &LauncherContext) -> Result<Vec<Box<dyn ContentVersion>>> {
        let mut ret: Vec<Box<dyn ContentVersion>> = Vec::new();
        let mut index = 0;
        loop {
            let query = if let Some(v) = for_version {
                #[cfg(feature="mod_loaders")]
                let loader = v.extra_data.components.get(0);
                let mut ret = vec![];
                #[cfg(feature="mod_loaders")]
                if let Some(loader) = loader {
                    ret.push(("modLoaderType", LOADER_TO_CURSEFORGE[&loader.name].to_string()))
                }
                if let Some(vers) = &v.extra_data.version {
                    ret.push(("gameVersion", vers.clone()));
                }
                ret.push(("index", index.to_string()));
                ret
            } else {
                vec![("index", index.to_string())]
            };
            let versions: DataWrapped<Vec<CurseforgeModFile>> = launcher.http_client.get(format!("https://api.curseforge.com/v1/mods/{}/files", self.id)).header("x-api-key", API_KEY).query(&query).send().await?.json().await?;
            let length = versions.data.len();
            let versions: Vec<_> = versions.data.into_iter().filter(|v|v.is_available).map(Box::new).map(|v|v as Box<dyn ContentVersion>).collect();
            index += 50;
            ret.extend(versions);
            if length < 50 {
                break;
            }
        }
        Ok(ret)
    }
    fn get_title(&self) -> String {
        self.name.clone()
    }
    fn get_description(&self) -> String {
        self.summary.clone()
    }
    async fn get_body(&self, launcher: &LauncherContext) -> Result<String> {
        let res: DataWrapped<String> = launcher.http_client.get(format!("https://api.curseforge.com/v1/mods/{}/description", self.id)).header("x-api-key", API_KEY).send().await?.json().await?;
        Ok(res.data)
    }
    fn get_icon_url(&self) -> Option<String> {
        Some(self.logo.url.clone())
    }
    fn get_urls(&self) -> HashMap<String, String> {
        let ret = [
            ("issues".to_string(), self.links.issues_url.clone()),
            ("source".to_string(), self.links.source_url.clone()),
            ("website".to_string(), self.links.website_url.clone()),
            ("wiki".to_string(), self.links.wiki_url.clone())
        ].into_iter().filter(|v|v.1.is_some()).map(|(k, v)|(k, v.unwrap())).collect();
        ret
    }
    fn get_screenshots(&self) -> Vec<Screenshot> {
        self.screenshots.iter().map(|v|{
            Screenshot {
                url: v.url.clone(),
                title: Some(v.title.clone()),
                description: Some(v.description.clone())
            }
        }).collect()
    }
    fn get_other_information(&self) -> HashMap<String, String> {
        let mut ret = HashMap::new();
        ret.insert("downloads".to_string(), self.download_count.to_string());
        ret.insert("authors".to_string(), self.authors.iter().map(|v|v.name.clone()).intersperse(", ".to_string()).collect());
        ret.insert("published".to_string(), self.date_created.clone());
        ret.insert("modified".to_string(), self.date_modified.clone());
        ret.insert("updated".to_string(), self.date_released.clone());
        ret
    }
    fn is_library(&self) -> bool {
        return self.categories.iter().any(|v|v.slug == "library-api");
    }
    fn kind(&self) -> ContentType {
        match self.primary_category_id {
            4471 => ContentType::ModPack,
            4546 => ContentType::Shader,
            6 => ContentType::Mod,
            12 => ContentType::ResourcePack,
            17 => ContentType::World,
            n => panic!("Unknown curseforge primary category id {n}!")
        }
    }
}

#[async_trait]
impl ContentVersion for CurseforgeModFile {
    fn get_version_file_url(&self) -> String {
        self.download_url.as_ref().unwrap().clone()
    }
    fn get_version_file_sha1(&self) -> String {
        self.hashes.iter().find(|v|v.algo == Algorithm::SHA1).unwrap().value.clone()
    }
    fn get_version_file_name(&self) -> String {
        self.file_name.clone()
    }
    async fn get_version_changelog(&self, launcher: &LauncherContext) -> Result<String> {
        let res: DataWrapped<String> = launcher.http_client.get(format!("https://api.curseforge.com/v1/mods/{}/files/{}/changelog", self.mod_id, self.id)).send().await?.json().await?;
        Ok(res.data)
    }
    fn get_version_number(&self) -> String {
        self.display_name.clone()
    }
    async fn list_dependencies(&self, launcher: &LauncherContext) -> Result<Vec<ContentDependency>> {
        let mut deps: Vec<ContentDependency> = Vec::new();
        for i in &self.dependencies {
            if RelationType::RequiredDependency != i.relation_type {
                continue;
            }
            let a = ContentDependency::Content(
                Box::new(CurseforgeMod::from_id(&i.mod_id.to_string(), &launcher).await?) as Box<dyn Content>
            );
            deps.push(a);
        }
        Ok(vec![])
    }
}

#[async_trait]
impl ContentService for CurseforgeContentService {
    async fn search_content(
        &self,
        name: String,
        skip: usize,
        limit: usize,
        kind: super::ContentType,
        sort_field: usize,
        for_version: Option<&MinecraftInstallation<'_>>,
        launcher: &LauncherContext
    ) -> Result<Vec<Box<dyn Content>>> {
        if ContentType::DataPack == kind {
            return Ok(vec![]);
        }
        let mut query = vec![
            ("gameId", "432".to_string()),
            ("classId", CONTENT_TYPE_TO_CURSEFORGE[&kind].to_string()),
            ("searchFilter", name),
            ("index", skip.to_string()),
            ("pageSize", limit.to_string()),
            ("sortField", (sort_field + 1).to_string()),
        ];
        if let Some(v) = for_version {
            if let Some(version) = &v.extra_data.version {
                query.push(("gameVersion", version.clone()));
            }
            #[cfg(feature="mod_loaders")]
            if let Some(loader) = v.extra_data.components.get(0) {
                query.push(("modLoaderType", LOADER_TO_CURSEFORGE[&loader.name].to_string()));
            }
        }
        let results: DataWrapped<Vec<CurseforgeMod>> = launcher.http_client.get("https://api.curseforge.com/v1/mods/search").header("x-api-key", API_KEY).query(&query).send().await?.json().await?;
        Ok(results.data.into_iter().map(|v|Box::new(v) as Box<dyn Content>).collect())
    }
    
    fn get_unsupported_content_types(&self) -> Vec<ContentType>  {
        vec![ContentType::DataPack]
    }
    
    fn get_sort_fields(&self) -> Vec<String> {
        vec!["featured".to_string(), "popularity".to_string(), "last_updated".to_string(), "name".to_string(), "author".to_string(), "total_downloads".to_string(), "category".to_string(), "game_version".to_string()]
    }
    
    fn get_default_sort_field(&self) -> String {
        "featured".to_string()
    }
    
    async fn get_content_version_from_file(&self, path: &BetterPath, launcher: &LauncherContext) -> Result<Option<Box<dyn ContentVersion>>> {
        let mut data = String::new();
        File::open(path).await?.read_to_string(&mut data).await?;
        let data = data.into_bytes().into_iter().filter(|v|[0x9, 0xa, 0xd, 0x20].contains(v)).collect::<Vec<_>>();
        let mm2 = murmur2(&data, 1);
        let res: Value = launcher.http_client.post("https://api.curseforge.com/v1/fingerprints").header("x-api-key", API_KEY).json(&json!({
            "fingerprints": [mm2]
        })).send().await?.json().await?;
        let exact_matches = res["data"]["exactMatches"].as_array().unwrap();
        if exact_matches.len() == 0 {
            Ok(None)
        } else {
            Ok(Some(Box::new(serde_json::from_value::<CurseforgeModFile>(exact_matches[0]["file"].clone())?) as Box<dyn ContentVersion>))
        }
    }

    async fn get_content_by_id(&self, id: &str, launcher: &LauncherContext) -> Result<Option<Box<dyn Content>>> {
        Ok(Some(Box::new(CurseforgeMod::from_id(id, launcher).await?)))
    }

    async fn get_content_version_by_id(&self, content_id: &str, id: &str, launcher: &LauncherContext) -> Result<Option<Box<dyn ContentVersion>>> {
        Ok(Some(Box::new(CurseforgeModFile::from_id(content_id, id, launcher).await?)))
    }
}
