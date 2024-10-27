use std::{collections::HashMap, fmt::Debug, sync::LazyLock};

use anyhow::{anyhow, Error, Result};
use async_trait::async_trait;
use futures_util::io::AllowStdIo;
use markdown_it::MarkdownIt;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tokio::fs::File;
use tokio_util::compat::TokioAsyncReadCompatExt;

use crate::{minecraft::version::MinecraftInstallation, utils::BetterPath, LauncherContext};

use super::{Content, ContentDependency, ContentService, ContentType, ContentVersion, Screenshot};

static MARKDOWN: LazyLock<MarkdownIt> = LazyLock::new(MarkdownIt::new);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SideEnv {
    Required,
    Optional,
    Unsupported
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Listed,
    Approved,
    Rejected,
    Draft,
    Unlisted,
    Archived,
    Processing,
    Unknown
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ProjectType {
    ModPack,
    Shader,
    Mod,
    ResourcePack
}

#[derive(Serialize, Deserialize)]
struct DonationURLs {
    id: String,
    platform: String,
    url: String
}

#[derive(Serialize, Deserialize)]
struct ModeratorMessage {
    message: String,
    body: Option<String>
}

#[derive(Serialize, Deserialize)]
struct License {
    id: String,
    name: String,
    url: Option<String>
}

#[derive(Serialize, Deserialize)]
struct Gallery {
    url: String,
    featured: bool,
    title: Option<String>,
    description: Option<String>,
    created: String,
    ordering: usize
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum VersionType {
    Release,
    Beta,
    Alpha
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RequestedStatus {
    Listed,
    Archived,
    Draft,
    Unlisted
}

#[derive(Serialize, Deserialize, Clone)]
struct Hashes {
    sha512: String,
    sha1: String
}

#[derive(Serialize, Deserialize)]
struct ModrinthProject {
    slug: String,
    title: String,
    description: String,
    categories: Vec<String>,
    client_side: SideEnv,
    server_side: SideEnv,
    body: Option<String>,
    additional_categories: Option<Vec<String>>,
    issues_url: Option<String>,
    source_url: Option<String>,
    wiki_url: Option<String>,
    discord_url: Option<String>,
    donation_urls: Option<Vec<DonationURLs>>,
    project_type: ProjectType,
    downloads: usize,
    icon_url: Option<String>,
    color: Option<usize>,
    id: String,
    team: String,
    moderator_message: Option<ModeratorMessage>,
    published: String,
    updated: String,
    approved: Option<String>,
    followers: usize,
    status: Status,
    license: License,
    versions: Vec<String>,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    gallery: Vec<Gallery>
}

#[derive(Serialize, Deserialize)]
struct SearchResults {
    hits: Vec<SearchResult>
}

#[derive(Serialize, Deserialize)]
struct SearchResult {
    slug: String,
    title: String,
    description: String,
    categories: Vec<String>,
    client_side: SideEnv,
    server_side: SideEnv,
    project_type: ProjectType,
    downloads: usize,
    icon_url: Option<String>,
    project_id: String,
    author: String,
    display_categories: Vec<String>,
    versions: Vec<String>,
    follows: usize,
    date_created: String,
    date_modified: String,
    latest_version: String,
    license: String,
    gallery: Vec<String>
}

#[derive(Serialize, Deserialize)]
struct Dependency {
    version_id: Option<String>,
    project_id: Option<String>,
    file_name: Option<String>,
    dependency_type: Option<DependencyType>
}

#[derive(Serialize, Deserialize, Clone)]
struct ModrinthFile {
    hashes: Hashes,
    url: String,
    filename: String,
    primary: bool,
    size: usize
}

#[derive(Serialize, Deserialize)]
struct ModrinthVersionModel {
    name: String,
    version_number: String,
    changelog: Option<String>,
    dependencies: Vec<Dependency>,
    game_versions: Vec<String>,
    version_type: VersionType,
    loaders: Vec<String>,
    featured: bool,
    status: Status,
    requested_status: Option<RequestedStatus>,
    id: String,
    project_id: String,
    author_id: String,
    date_published: String,
    downloads: usize,
    files: Vec<ModrinthFile>
}

#[allow(unused)]
pub(crate) struct ModrinthContentService;

impl Debug for ModrinthProject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModrinthProject({})", self.slug)
    }
}

impl Debug for ModrinthContentVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModrinthContentVersion({})", self.0.name)
    }
}

struct ModrinthContentVersion(ModrinthVersionModel, ModrinthFile);

impl ModrinthProject {
    async fn from_id(id: &str, launcher: &LauncherContext) -> Result<Self> {
        Ok(launcher.http_client.get(format!("https://api.modrinth.com/v2/project/{id}")).send().await?.json().await?)
    }
}

impl ModrinthContentVersion {
    async fn from_id(id: &str, launcher: &LauncherContext) -> Result<Self> {
        Ok(Self::new(launcher.http_client.get(format!("https://api.modrinth.com/v2/version/{id}")).send().await?.json().await?))
    }
}

#[async_trait]
impl Content for ModrinthProject {
    /**
     * List versions.
     * @param forVersion The Minecraft version you download for.
     * @throws RequestError
     */
    async fn list_downloadable_versions(&self, for_version: Option<&MinecraftInstallation<'_>>, launcher: &LauncherContext) -> Result<Vec<Box<dyn ContentVersion>>> {
        let query = if let Some(v) = for_version {
            #[cfg(feature="mod_loaders")]
            let loaders: String = serde_json::to_string(&v.extra_data.components.iter().map(|v|v.name.clone()).collect::<Vec<String>>()).unwrap();
            let mut ret = vec![#[cfg(feature="mod_loaders")]("loaders", loaders)];
            if let Some(vers) = &v.extra_data.version {
                ret.push(("game_versions", format!("[\"{vers}\"]")));
            }
            ret
        } else {
            vec![]
        };
        let versions: Vec<ModrinthVersionModel> = launcher.http_client.get(format!("https://api.modrinth.com/v2/project/{}/version", self.slug)).query(&query).send().await?.json().await?;
        Ok(versions.into_iter().map(|v|Box::new(ModrinthContentVersion::new(v)) as Box<dyn ContentVersion>).collect())
    }
    fn get_title(&self) -> String {
        self.title.clone()
    }
    fn get_description(&self) -> String {
        self.description.clone()
    }
    async fn get_body(&self, _: &LauncherContext) -> Result<String> {
        Ok(MARKDOWN.parse(&self.body.clone().unwrap_or_default()).render())
    }
    fn get_icon_url(&self) -> Option<String> {
        self.icon_url.clone()
    }
    fn get_urls(&self) -> HashMap<String, String> {
        let mut ret = HashMap::new();
        if let Some(v) = &self.wiki_url {
            ret.insert("wiki".to_string(), v.clone());
        }
        if let Some(v) = &self.issues_url {
            ret.insert("issues".to_string(), v.clone());
        }
        if let Some(v) = &self.source_url {
            ret.insert("source".to_string(), v.clone());
        }
        if let Some(v) = &self.discord_url {
            ret.insert("discord".to_string(), v.clone());
        }
        for v in self.donation_urls.iter().flatten() {
            ret.insert(format!("donation.{}", v.platform), v.url.clone());
        }
        ret
    }
    fn get_screenshots(&self) -> Vec<Screenshot> {
        self.gallery.iter().map(|v|{
            Screenshot {
                url: v.url.clone(),
                title: v.title.clone(),
                description: v.description.clone()
            }
        }).collect()
    }
    fn get_other_information(&self) -> HashMap<String, String> {
        let mut ret = HashMap::new();
        ret.insert("downloads".to_string(), self.downloads.to_string());
        ret.insert("followers".to_string(), self.followers.to_string());
        ret.insert("license".to_string(), self.license.name.clone());
        ret.insert("published".to_string(), self.published.clone());
        ret.insert("updated".to_string(), self.updated.clone());
        ret
    }
    fn is_library(&self) -> bool {
        return self.categories.contains(&"library".to_string());
    }
    fn kind(&self) -> ContentType {
        match self.project_type {
            ProjectType::ModPack => ContentType::ModPack,
            ProjectType::Shader => ContentType::Shader,
            ProjectType::Mod => ContentType::Mod,
            ProjectType::ResourcePack => ContentType::ResourcePack,
        }
    }
}

impl ModrinthContentVersion {
    fn new(model: ModrinthVersionModel) -> Self {
        let file = model.files.iter().find(|v|v.primary).unwrap_or(&model.files[0]).clone();
        Self(model, file)
    }
}

#[async_trait]
impl ContentVersion for ModrinthContentVersion {
    fn get_version_file_url(&self) -> String {
        self.1.url.clone()
    }
    fn get_version_file_sha1(&self) -> String {
        self.1.hashes.sha1.clone()
    }
    fn get_version_file_name(&self) -> String {
        self.1.filename.clone()
    }
    async fn get_version_changelog(&self, _: &LauncherContext) -> Result<String> {
        Ok(MARKDOWN.parse(&self.0.changelog.as_ref().unwrap_or(&"".to_string())).render())
    }
    fn get_version_number(&self) -> String {
        self.0.version_number.clone()
    }
    async fn list_dependencies(&self, launcher: &LauncherContext) -> Result<Vec<ContentDependency>> {
        let mut deps: Vec<ContentDependency> = Vec::new();
        for i in &self.0.dependencies {
            if let Some(DependencyType::Incompatible | DependencyType::Embedded) | None = i.dependency_type {
                continue;
            }
            if let Some(v) = &i.version_id {
                let a = ContentDependency::ContentVersion(
                    Box::new(ModrinthContentVersion::from_id(&v, &launcher).await?)
                );
                deps.push(a);
            }
            if let Some(v) = &i.project_id {
                let a = ContentDependency::Content(
                    Box::new(ModrinthProject::from_id(v, &launcher).await?)
                );
                deps.push(a);
            }
        }
        Ok(deps)
    }
}

#[async_trait]
impl ContentService for ModrinthContentService {
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
        if let ContentType::World = kind {
            return Ok(vec![]);
        }
        let mut facets = vec![
            vec![format!("project_type:{}", match kind {
                ContentType::ModPack => "modpack",
                ContentType::Shader => "shader",
                ContentType::Mod => "mod",
                ContentType::ResourcePack => "resourcepack",
                ContentType::DataPack => "datapack",
                ContentType::World => panic!("???"),
            })],
        ];
        if let Some(v) = for_version {
            if let Some(version) = &v.extra_data.version {
                facets.push(vec![format!("versions:{version}")]);
            }
            #[cfg(feature="mod_loaders")]
            let loaders: Vec<String> = v.extra_data.components.iter().map(|v| {
                format!("categories:{}", v.name)
            }).collect();
            #[cfg(feature="mod_loaders")]
            facets.push(loaders);
        }
        let results: SearchResults = launcher.http_client.get("https://api.modrinth.com/v2/search").query(&[
            ("query", name),
            ("facets", serde_json::to_string(&facets).unwrap()),
            ("offset", skip.to_string()),
            ("limit", limit.to_string()),
            ("index", self.get_sort_fields()[sort_field].clone()),
        ]).send().await?.json().await?;
        let res = futures::future::join_all(results.hits.iter().map(|v|ModrinthProject::from_id(&v.project_id, &launcher))).await;
        let errors: Vec<&Error> = res.iter().filter(|v|v.is_err()).map(|v|v.as_ref().unwrap_err()).collect();
        if !errors.is_empty() {
            Err(anyhow!(errors.iter().map(|v|format!("{v} {}\n", v.root_cause())).collect::<String>()).into())
        } else {
            Ok(res.into_iter().map(|v|Box::new(v.unwrap()) as Box<dyn Content>).collect())
        }
    }
    
    fn get_unsupported_content_types(&self) -> Vec<ContentType>  {
        vec![ContentType::World]
    }
    
    fn get_sort_fields(&self) -> Vec<String> {
        vec!["newest".to_string(), "updated".to_string(), "relevance".to_string(), "downloads".to_string(), "follows".to_string()]
    }
    
    fn get_default_sort_field(&self) -> String {
        "relevance".to_string()
    }
    
    async fn get_content_version_from_file(&self, path: &BetterPath, launcher: &LauncherContext) -> Result<Option<Box<dyn ContentVersion>>> {
        let mut sha1 = AllowStdIo::new(Sha1::new());
        futures_util::io::copy(File::open(path).await?.compat(), &mut sha1).await?;
        let res = launcher.http_client.get(format!("https://api.modrinth.com/v2/version/version_file/{:X}?algorithm=sha1", sha1.into_inner().finalize())).send().await?;
        if res.status() == StatusCode::NOT_FOUND {
            Ok(None)
        } else {
            Ok(Some(Box::new(ModrinthContentVersion::new(res.json().await?))))
        }
    }

    async fn get_content_by_id(&self, id: &str, launcher: &LauncherContext) -> Result<Option<Box<dyn Content>>> {
        Ok(Some(Box::new(ModrinthProject::from_id(id, launcher).await?)))
    }

    async fn get_content_version_by_id(&self, _content_id: &str, id: &str, launcher: &LauncherContext) -> Result<Option<Box<dyn ContentVersion>>> {
        Ok(Some(Box::new(ModrinthContentVersion::from_id(id, launcher).await?)))
    }
}
