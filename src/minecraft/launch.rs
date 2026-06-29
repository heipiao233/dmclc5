//! Things about launching Minecraft.

use std::{collections::HashMap, ffi::{OsStr, OsString}, fs::File, iter::once, path::PathBuf};

use anyhow::{Ok, Result};
use either::Either;
use osstrtools_fix::{Bytes, OsStringTools};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{minecraft::login::AccountTrait, utils::{DownloadAllMessage, PATH_DELIMITER, check_rules, check_rules_no_option, get_bits, get_os}};

use super::{login::Account, schemas::{Argument, Library, OneOrMoreArguments, VersionJSON}, version::MinecraftInstallation};

impl MinecraftInstallation {
    /// Generate the launch arguments.
    /// Please refresh account.
    /// Please run [super::version::DMCLCExtraData::before_command] before launching.
    /// Please use [super::version::DMCLCExtraData::with_java].
    /// Please set the work dir to [Self::get_cwd].
    pub async fn launch_args(&self, account: &Account, download_channel: mpsc::UnboundedSender<DownloadAllMessage>) -> Result<Vec<OsString>> {
        account.prepare_launch(&self.version_launch_work_dir, &self.launcher).await?;
        self.complete_files(false, false, download_channel).await?;
        self.unzip_natives()?;
        let mut args = vec![];
        let cp = self.gen_classpath().join(PATH_DELIMITER.bytes_as_os_str());
        match &self.obj {
            VersionJSON::Old { base, minecraft_arguments } => {
                let mut lib = OsString::from("-Djava.library.path=");
                lib.push((self.version_root.clone() / "natives").0.into_os_string());
                args.push(lib);
                args.push(OsString::from("-cp"));
                args.push(cp.clone());
                args.extend(self.extra_data.extra_jvm_arguments.clone().into_iter().flatten());
                args.push(OsString::from(&base.main_class));
                for i in minecraft_arguments.split(" ") {
                    args.extend(self.transform_arg(&Argument::String(i.to_string()), &cp, account));
                }
                args.extend(self.extra_data.extra_game_arguments.clone().into_iter().flatten());
            },
            VersionJSON::New { base: _, arguments } => {
                if let Some(jvm) = &arguments.jvm {
                    for i in jvm {
                        args.extend(self.transform_arg(i, &cp, account));
                    }
                }

                args.extend(account.get_launch_jvmargs(self, &self.launcher).await?);
                args.extend(self.extra_data.extra_jvm_arguments.clone().into_iter().flatten());
                args.push(OsString::from(self.obj.get_base().main_class.clone()));

                if let Some(game) = &arguments.game {
                    for i in game {
                        args.extend(self.transform_arg(i, &cp, account));
                        args.extend(self.extra_data.extra_jvm_arguments.clone().into_iter().flatten());
                    }
                }
            }
        }
        Ok(args)
    }

    fn unzip_natives(&self) -> Result<()> {
        for i in &self.obj.get_base().libraries {
            if let Library::VanillaNatives(n) = i {
                if !check_rules(&i.get_base().rules) {
                    continue;
                }
                let env = &n.natives[&get_os()].replace("${arch}", &get_bits());
                if !n.downloads.classifiers.contains_key(env) {
                    continue;
                }
                let native = &n.downloads.classifiers[env];
                let libpath = self.launcher.root_path.clone() / "libraries" / &native.path;
                zip::ZipArchive::new(File::open(libpath)?)?.extract(self.version_root.clone() / "natives")?;
            }
        }
        Ok(())
    }

    /// Get the correct work dir.
    pub fn get_cwd(&self) -> PathBuf {
        self.version_launch_work_dir.0.clone()
    }

    fn transform_arg(&self, arg: &Argument, cp: &OsStr, account: &Account) -> Vec<OsString> {
        let auth_uuid = account.get_uuid().simple().to_string();
        let game_dir = self.version_launch_work_dir.0.as_os_str();
        let assets_root = (self.launcher.root_path.clone() / "assets");
        let natives_dir = (self.version_root.clone() / "natives");
        let libraries_dir = (self.launcher.root_path.clone() / "libraries");
        let args = std::iter::once(arg)
            .filter_map(|arg| match arg {
                Argument::String(s) => Some(Either::Left(once(s))),
                Argument::Conditional { rules, value: _ } if !check_rules_no_option(rules) => None,
                Argument::Conditional { rules: _, value: OneOrMoreArguments::One(v) } => {
                    Some(Either::Left(once(v)))
                }
                Argument::Conditional { rules: _, value: OneOrMoreArguments::More(v) } => {
                    Some(Either::Right(v.iter()))
                }
            })
            .flatten()
            .map(|arg| account.replace_launch_game_arg(arg))
            .map(OsString::from)
            .map(|arg| arg.replace("${version_name}", self.name.as_str())
                .replace("${game_directory}", game_dir)
                .replace("${assets_root}", assets_root.0.as_os_str())
                .replace("${assets_index_name}", self.obj.get_base().assets.as_str())
                .replace("${auth_uuid}", auth_uuid.as_str())
                .replace("${version_type}", "DMCL5")
                .replace("${natives_directory}", natives_dir.0.as_os_str())
                .replace("${launcher_name}", self.launcher.name.as_str())
                .replace("${launcher_version}", "0.1")
                .replace("${library_directory}", libraries_dir.0.as_os_str())
                .replace("${classpath_separator}", PATH_DELIMITER)
                .replace("${classpath}", cp));

        args.collect()
    }

    fn gen_classpath(&self) -> Vec<OsString> {
        let mut ret: Vec<OsString> = vec![];
        for lib in &self.obj.get_base().libraries {
            if !check_rules(&lib.get_base().rules) {
                continue;
            }
            if let Library::VanillaNatives(_) = lib {
                continue;
            }
            let path = (self.launcher.root_path.clone() / "libraries" / &lib.get_base().name.to_path()).0.into_os_string();
            if !ret.contains(&path) {
                ret.push(path);
            }
        }
        ret.push((self.version_root.clone() / format!("{}.jar", self.name)).0.into_os_string());
        ret
    }
}
