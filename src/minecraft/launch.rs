//! Things about launching Minecraft.

use std::{ffi::{OsStr, OsString}, fs::File, iter::{self, once}, path::Path};

use anyhow::{Ok, Result};
use either::Either;
use osstrtools_fix::{Bytes, OsStringTools};
use tokio::sync::mpsc;

use crate::{minecraft::login::AccountTrait, utils::{DownloadAllMessage, PATH_DELIMITER, check_rules, check_rules_no_option, get_bits, get_os}};

use super::{login::Account, schemas::{Argument, Library, OneOrMoreArguments, VersionJSON}, version::MinecraftInstallation};

impl MinecraftInstallation<'_, '_> {
    /// Generate the launch arguments.
    /// Please refresh account.
    /// Please run [super::version::DMCLCExtraData::before_command] before launching.
    /// Please use [super::version::DMCLCExtraData::with_java].
    /// Please set the work dir to [Self::get_cwd].
    pub async fn launch_args(&self, account: &Account, download_channel: mpsc::UnboundedSender<DownloadAllMessage>) -> Result<Vec<OsString>> {
        account.prepare_launch(&self.version_launch_work_dir, self.prefix.config).await?;
        self.complete_files(false, false, download_channel).await?;
        self.unzip_natives()?;
        let mut args = vec![];
        let cp = self.gen_classpath().join(PATH_DELIMITER.bytes_as_os_str());
        match &self.obj {
            VersionJSON::Old { base, minecraft_arguments } => {
                let mut lib = OsString::from("-Djava.library.path=");
                lib.push(self.version_root.join("natives").into_os_string());
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

                args.extend(account.get_launch_jvmargs(self.prefix.config).await?);
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
                let libpath = self.prefix.config.get_libraries_path(&native.path);
                zip::ZipArchive::new(File::open(libpath)?)?.extract(self.version_root.join("natives"))?;
            }
        }
        Ok(())
    }

    /// Get the correct work dir.
    pub fn get_cwd(&self) -> &Path {
        &self.version_launch_work_dir
    }

    fn transform_arg(&self, arg: &Argument, cp: &OsStr, account: &Account) -> Vec<OsString> {
        let auth_uuid = account.get_uuid().simple().to_string();
        let game_dir = self.version_launch_work_dir.as_os_str();
        let assets_root = &self.prefix.config.assets_path;
        let natives_dir = self.version_root.join("natives");
        let libraries_dir = &self.prefix.config.libraries_path;
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
                .replace("${assets_root}", assets_root.as_os_str())
                .replace("${assets_index_name}", self.obj.get_base().assets.as_str())
                .replace("${auth_uuid}", auth_uuid.as_str())
                .replace("${version_type}", "DMCL5")
                .replace("${natives_directory}", natives_dir.as_os_str())
                .replace("${launcher_name}", self.prefix.config.name.as_str())
                .replace("${launcher_version}", "0.1")
                .replace("${library_directory}", libraries_dir.as_os_str())
                .replace("${classpath_separator}", PATH_DELIMITER)
                .replace("${classpath}", cp));

        args.collect()
    }

    fn gen_classpath(&self) -> Vec<OsString> {
        self.obj.get_base().libraries.iter()
            .filter(|lib| check_rules(&lib.get_base().rules))
            .filter(|lib| !matches!(lib, Library::VanillaNatives(_)))
            .map(|lib| (self.prefix.config.get_libraries_path(lib.get_base().name.to_path())).into_os_string())
            .chain(iter::once(self.version_root.join(format!("{}.jar", self.name)).into_os_string()))
            .collect()
    }
}
