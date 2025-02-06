//! Things about launching Minecraft.

use std::{collections::HashMap, ffi::{OsStr, OsString}, fs::File, path::PathBuf};

use anyhow::{Ok, Result};
use osstrtools_fix::{Bytes, OsStringTools};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::utils::{check_rules, check_rules_no_option, get_bits, get_os, DownloadAllMessage, PATH_DELIMITER};

use super::{login::Account, schemas::{Argument, Library, OneOrMoreArguments, VersionJSON}, version::MinecraftInstallation};

impl <'a> MinecraftInstallation<'a> {
    /// Generate the launch arguments.
    /// Please run [super::version::DMCLCExtraData::before_command] before launching.
    /// Please use [super::version::DMCLCExtraData::with_java].
    /// Please set the work dir to [Self::get_cwd].
    pub async fn launch_args(&self, account: &mut dyn Account, download_channel: mpsc::UnboundedSender<DownloadAllMessage>) -> Result<Vec<OsString>> {
        if !account.is_initialized() || !account.check(&self.launcher).await {
            account.login(&self.launcher).await?;
        }
        account.prepare_launch(&self.version_launch_work_dir, &self.launcher).await?;
        self.complete_files(false, false, download_channel).await?;
        self.unzip_natives()?;
        let mut args = vec![];
        let cp = self.gen_classpath().join(PATH_DELIMITER.bytes_as_os_str());
        let account_game_args = account.get_launch_game_args(&self.launcher).await;
        match &self.obj {
            VersionJSON::Old { base, minecraft_arguments } => {
                let mut lib = OsString::from("-Djava.library.path=");
                lib.push((&self.version_root / "natives").0.into_os_string());
                args.push(lib);
                args.push(OsString::from("-cp"));
                args.push(cp.clone());
                args.extend(self.extra_data.extra_jvm_arguments.clone().into_iter().flatten());
                args.push(OsString::from(&base.main_class));
                for i in minecraft_arguments.split(" ") {
                    args.extend(self.transform_arg(&Argument::String(i.to_string()), &cp, &account_game_args, &account.get_uuid()));
                }
                args.extend(self.extra_data.extra_game_arguments.clone().into_iter().flatten());
            },
            VersionJSON::New { base: _, arguments } => {
                if let Some(jvm) = &arguments.jvm {
                    for i in jvm {
                        args.extend(self.transform_arg(i, &cp, &account_game_args, &account.get_uuid()));
                    }
                }

                args.extend(account.get_launch_jvmargs(self, &self.launcher).await?);
                args.extend(self.extra_data.extra_jvm_arguments.clone().into_iter().flatten());
                args.push(OsString::from(self.obj.get_base().main_class.clone()));

                if let Some(game) = &arguments.game {
                    for i in game {
                        args.extend(self.transform_arg(i, &cp, &account_game_args, &account.get_uuid()));
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
                let libpath = &self.launcher.root_path / "libraries" / native.path.clone();
                zip::ZipArchive::new(File::open(libpath)?)?.extract(&self.version_root / "natives")?;
            }
        }
        Ok(())
    }

    /// Get the correct work dir.
    pub fn get_cwd(&self) -> PathBuf {
        self.version_launch_work_dir.0.clone()
    }

    fn transform_arg(&self, arg: &Argument, cp: &OsStr, account_args: &HashMap<String, String>, account_uuid: &Uuid) -> Vec<OsString> {
        let mut args = vec![];
        match arg {
            Argument::String(s) => args.push(OsString::from(s)),
            Argument::Conditional{rules, value} => {
                if !check_rules_no_option(rules) {
                    return vec![];
                }
                match value {
                    OneOrMoreArguments::One(v) => args.push(OsString::from(v)),
                    OneOrMoreArguments::More(v) => args.extend(v.iter().map(OsString::from)),
                }
            }
        }

        for i in &mut args {
            *i = i.clone().replace("${version_name}", self.name.as_str())
                .replace("${game_directory}", self.version_launch_work_dir.0.as_os_str())
                .replace("${assets_root}", (&self.launcher.root_path / "assets").0.as_os_str())
                .replace("${assets_index_name}", self.obj.get_base().assets.as_str())
                .replace("${auth_uuid}", account_uuid.simple().to_string().as_str())
                .replace("${version_type}", "DMCL5")
                .replace("${natives_directory}", (&self.version_root / "natives").0.as_os_str())
                .replace("${launcher_name}", "DMCLC5")
                .replace("${launcher_version}", "0.1")
                .replace("${library_directory}", (&self.launcher.root_path / "libraries").0.as_os_str())
                .replace("${classpath_separator}", PATH_DELIMITER)
                .replace("${classpath}", cp);
            for (k, v) in account_args {
                *i = i.clone().replace(k.as_str(), v.as_str());
            }
        }

        return args;
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
            let path = (&self.launcher.root_path / "libraries" / &lib.get_base().name.to_path()).0.into_os_string();
            if !ret.contains(&path) {
                ret.push(path);
            }
        }
        ret.push((&self.version_root / format!("{}.jar", self.name)).0.into_os_string());
        ret
    }
}