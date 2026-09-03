//! Implementation of [ComponentInstaller] for Forge-like installers.

use std::{collections::HashMap, ffi::OsString, io::Read, path::PathBuf, process::Stdio};

use anyhow::{anyhow, Result};
use fs_extra::dir::CopyOptions;
use futures::{StreamExt, TryStreamExt, stream};
use osstrtools_fix::Bytes;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use tokio::{fs, process::Command, sync::mpsc};

#[cfg(feature = "mod_loaders")]
use crate::components::mods::ModLoader;
use crate::{LauncherContext, minecraft::{schemas::{Library, VersionJSON}, version::MinecraftInstallation}, utils::{BetterPathBuf, DownloadAllMessage, PATH_DELIMITER, check_hash, download_all, download_res, download_to_writer, expand_maven_id, maven_coord::ArtifactCoordinate, merge_version_json}};

use super::ComponentInstallerTrait;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DataEntry {
    client: String,
    server: String
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Side {
    Server,
    Client
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Processor {
    #[serde(default)]
    sides: Vec<Side>,
    jar: ArtifactCoordinate,
    classpath: Vec<ArtifactCoordinate>,
    args: Vec<String>,
    #[serde(default)]
    outputs: HashMap<String, String>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct InstallerProfileNew {
    data: HashMap<String, DataEntry>,

    processors: Vec<Processor>,
    libraries: Vec<Library>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct InstallerInformation {
    #[serde(rename = "filePath")]
    file_path: String,
    path: ArtifactCoordinate
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct InstallerProfileOld {
    #[serde(rename = "versionInfo")]
    version_info: VersionJSON,
    install: InstallerInformation
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum InstallerProfile {
    Old(InstallerProfileOld),
    New(InstallerProfileNew)
}

/// A Forge-like installer.
pub trait ForgeLikeInstallerTrait {
    /// Returns true if it supports old install_profile.json. (Installer for Forge <= 1.12)
    const SUPPORTS_OLDER_VERSION: bool;
    /// Returns the Maven group url in the repository.
    const MAVEN_GROUP_URL: &'static str;
    /// Get the mod loaders it provides.
    #[cfg(feature = "mod_loaders")]
    fn get_mod_loaders(version: &str, launcher: &LauncherContext) -> Vec<ModLoader>;
    /// Find the component in the [VersionJSON]. Returns the component version.
    fn find_in_version(mc: &VersionJSON) -> Option<String>;
    /// Returns the Maven archive base name in the repository.
    fn get_archive_base_name(mc_version: &str) -> String;
    /// Returns true if the given component version matches the Minecraft version.
    fn match_version(loader: &str, mc: &str) -> bool;
}

/// A type mark for Forge-like installers.
#[derive(Clone, Copy)]
pub struct ForgeLikeInstaller<T: ForgeLikeInstallerTrait>(pub(in crate::components::install) T);

impl <T: ForgeLikeInstallerTrait> ComponentInstallerTrait for ForgeLikeInstaller<T> {

    #[cfg(feature = "mod_loaders")]
    async fn get_mod_loaders(&self, version: &str, launcher: &LauncherContext) -> Result<Vec<ModLoader>> {
        Ok(T::get_mod_loaders(version, launcher))
    }

    async fn get_suitable_loader_versions(&self, mc: &MinecraftInstallation) -> Result<Vec<String>>  {
        let version = mc.extra_data.version.as_ref().unwrap().clone();
        let mut version_split = version.split(".");
        version_split.next();
        let (major, minor): (u8, u8) = (version_split.next().unwrap().parse()?, version_split.next().unwrap_or("0").parse()?);
        if major < 5 || (major == 5 && minor != 2) {
            return Ok(vec![]);
        }
        let res = reqwest::get(format!("{}/{}/maven-metadata.xml", T::MAVEN_GROUP_URL, T::get_archive_base_name(&version))).await?.text().await?;
        let val = xmltree::Element::parse(res.as_bytes())?;
        Ok(val.get_child("versioning").unwrap()
            .get_child("versions").unwrap()
            .children.iter().map(|v|v.as_element().unwrap().get_text().unwrap().to_string())
            .filter(|v| T::match_version(&v, &mc.extra_data.version.as_ref().unwrap())).collect())
    }

    async fn install(&self, mc: &mut MinecraftInstallation, version: &str, download_channel: mpsc::UnboundedSender<DownloadAllMessage>) -> Result<()> {
        let mcver = mc.extra_data.version.as_ref().unwrap().clone();
        let mut tmpfile = tokio::fs::File::from_std(tempfile::tempfile()?);
        let url = format!("{}/{1}/{version}/{}-{version}-installer.jar", T::MAVEN_GROUP_URL, T::get_archive_base_name(&mcver));
        download_to_writer(&url, &mut tmpfile).await?;
        let installer_dir = &BetterPathBuf(tempfile::tempdir()?.keep());
        zip::ZipArchive::new(tmpfile.into_std().await)?.extract(installer_dir)?;
        let metadata: InstallerProfile = serde_json::from_reader(std::fs::File::open(installer_dir.clone() / "install_profile.json")?)?;
        let result;
        match metadata {
            InstallerProfile::New(metadata) => {
                if metadata.data.contains_key("MOJMAPS") {
                    let id = &metadata.data["MOJMAPS"].client;
                    let id: String = id.chars().skip(1).take(id.len() - 2).collect();
                    let path = mc.launcher.root_path.clone() / "libraries" / expand_maven_id(&id);
                    download_res(mc.obj.get_base().downloads.client_mappings.as_ref().unwrap(), path.as_ref()).await?;
                }
                let maven_dir = installer_dir.clone() / "maven";
                if let Ok(f) = fs::metadata(&maven_dir).await && f.is_dir() {
                    fs_extra::dir::copy(&maven_dir, mc.launcher.root_path.clone() / "libraries", &CopyOptions::new().content_only(true))?;
                }
                let mut res = mc.libraries(&metadata.libraries, false);
                let target = &mc.obj;
                let source: VersionJSON = serde_json::from_reader(std::fs::File::open(installer_dir.clone() / "version.json")?)?;
                result = merge_version_json(target, &source)?;
                res.extend(mc.libraries(&source.get_base().libraries, false));
                download_all(
                    res, download_channel, mc.launcher.download_threads_per_file,
                    mc.launcher.download_parallel_files, mc.launcher.download_retries,
                    mc.launcher.bmclapi_mirror.clone()
                ).await?;

                stream::iter(&metadata.processors)
                    .filter(|p| futures::future::ready(!p.args.contains(&"DOWNLOAD_MOJMAPS".to_string())))
                    .filter(|p| futures::future::ready(p.sides.contains(&Side::Client) || p.sides.is_empty()))
                    .filter(|p| async {
                        !stream::iter(&p.outputs)
                            .map(|(name, hash)| (transform_arguments(name, &installer_dir, &mc, &metadata), transform_arguments(hash, &installer_dir, &mc, &metadata)))
                            .all(async |(name, hash)| check_hash::<Sha1>(
                                &PathBuf::from(name),
                                &hash.into_string().unwrap(),
                                0
                            ).await).await
                    })
                    .map(|p| (p, mc.launcher.root_path.clone() / "libraries" / p.jar.to_path()))
                    .map(|(p, jar)| (stream::iter(p.classpath.iter().map(|i|mc.launcher.root_path.clone() / "libraries" / i.to_path())
                        .chain(std::iter::once(jar.clone()))
                        .map(|BetterPathBuf(i)|i)
                        .map(PathBuf::into_os_string)).enumerate()
                        .fold(OsString::new(), |mut acc, (idx, item)| async move {
                            if idx != 0 {
                                acc.push(PATH_DELIMITER.bytes_as_os_str());
                            }
                            acc.push(item);
                            acc
                        }), jar))
                    .then(async |(classpath, jar)|
                        Ok::<_, anyhow::Error>(vec![
                            OsString::from("-cp"),
                            classpath.await,
                            get_main_class(&jar)?
                        ])
                    )
                    .then(async |args|
                        Command::new("java")
                            .args(args?)
                            .stdout(Stdio::inherit())
                            .stderr(Stdio::inherit())
                            .stdin(Stdio::null())
                            .spawn()?
                            .wait().await?.success().then(||()).ok_or(anyhow!(t!("A processor failed to run!")))
                    )
                    .try_collect::<()>().await?
            },
            InstallerProfile::Old(metadata) => {
                if !T::SUPPORTS_OLDER_VERSION {
                    return Ok(());
                }
                let target = &mc.obj;
                let source: VersionJSON = metadata.version_info;
                result = merge_version_json(target, &source)?;
                tokio::fs::copy(installer_dir.clone() / metadata.install.file_path, mc.launcher.root_path.clone() / "libraries" / metadata.install.path.to_path()).await?;
            }
        }
        serde_json::to_writer(&std::fs::File::create(mc.version_root.clone() / (mc.name.to_string() + ".json"))?, &result)?;
        mc.obj = result;
        Ok(())
    }

    fn find_in_version(&self, v: &VersionJSON) -> Option<String>  {
        T::find_in_version(v)
    }
}

fn get_main_class(path: &BetterPathBuf) -> Result<OsString> {
    let mut file = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    let mut manifest = file.by_name("META-INF/MANIFEST.MF")?;
    let mut manifest_content = String::new();
    manifest.read_to_string(&mut manifest_content)?;
    let line = manifest_content.lines()
        .find(|l|l.starts_with("Main-Class:"))
        .ok_or::<anyhow::Error>(anyhow!("No main class in processor jar!").into())? // TODO: i18n
        .strip_prefix("Main-Class:").unwrap().trim();
    Ok(OsString::from(line))
}

fn transform_arguments(arg: &str, installer_path: &BetterPathBuf, mc: &MinecraftInstallation, metadata: &InstallerProfileNew) -> OsString {
    let content = arg.chars().skip(1).take(arg.len() - 2).collect::<String>();
    if arg.starts_with("{") && arg.ends_with("}") {
        return match content.as_str() {
            "SIDE" => OsString::from("client"),
            "MINECRAFT_JAR" => (mc.version_root.clone() / (mc.name.clone() + ".jar")).0.into_os_string(),
            "BINPATCH" => (installer_path.clone() / "data/client.lzma").0.into_os_string(),
            other => transform_arguments(&metadata.data[other].client, installer_path, mc, metadata)
        };
    } else if arg.starts_with("[") && arg.ends_with("]") {
        return (mc.launcher.root_path.clone() / "libraries" / expand_maven_id(&content)).0.into_os_string();
    }
    return arg.into();
}
