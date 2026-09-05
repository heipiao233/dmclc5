use std::{path::PathBuf, process::Stdio};

use anyhow::Result;
use dmclc5::{LauncherConfig, minecraft::{login::{Account::self, offline::OfflineAccount}, prefix::MinecraftPrefix, schemas::VersionList}, utils::{DownloadAllMessage, DownloadEvent}};
use tokio::{process::Command, sync::mpsc};

async fn handle_msg(msg: DownloadAllMessage, count: &mut usize) {
    match msg {
        (c, DownloadEvent::ContentLength(len)) => {
            println!("{} start: {len}", c);
        },
        (c, DownloadEvent::Finish(_)) => {
            println!("{} end ({count})", c);
            *count -= 1;
        },
        (_, DownloadEvent::Start) => {
            *count += 1;
            println!("{count}");
        },
        (_c, DownloadEvent::Progress(_prog)) => {
            // println!("{} fetching: {prog}", c.0.display());
        },
        (c, DownloadEvent::Retry(_)) => {
            println!("{} retrying", c);
        },
    }
}

async fn real_main() -> Result<()> {
    let vers = VersionList::get_list().await?;
    let mut config = LauncherConfig::new("Test".to_string(), PathBuf::from("./test/assets"), PathBuf::from("./test/libraries")/*, "71dd081b-dc92-4d36-81ac-3a2bde5527ba".to_string()*/)?;
    config.bmclapi_mirror = Some("bmclapi2.bangbang93.com".into());
    let prefix: MinecraftPrefix = MinecraftPrefix::new(PathBuf::from("./test")).unwrap();
    let (tx, mut rx) = mpsc::channel(1000);
    let message_handler = async move {
        let mut count = 0;
        while let Some(next) = rx.recv().await {
            handle_msg(next, &mut count).await;
        }
    };
    let mc = vers.find_by_id("26.2").unwrap().install(&prefix, &config, "26.2", tx);
    let mc = tokio::join!(message_handler, mc).1?;
    // let msa = MicrosoftAccount::start_auth(&launcher).await?;
    // println!(
    //     "Open this URL in your browser:\n{}\nand enter the code: {}",
    //     msa.verification_uri().to_string(),
    //     msa.user_code().secret().to_string()
    // );
    // let account: Account = MicrosoftAccount::login(&launcher, &msa).await?.into();
    // let account = account.check(&launcher).await?;
    let account = Account::OfflineAccount(OfflineAccount::new("heipiao".to_string()));
    if let Some(c) = &mc.extra_data.before_command {
        let mut command = c.split(" ");
        Command::new(command.next().unwrap())
            .args(command)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdin(Stdio::null())
            .current_dir(mc.get_cwd())
            .spawn()?.wait().await?;
    }
    let (tx, mut rx) = mpsc::channel(1000);
    let msg_handler = async move {
        let mut count = 0;
        while let Some(next) = rx.recv().await {
            handle_msg(next, &mut count).await;
        }
    };
    let args = mc.launch_args(&account, tx);
    let args = tokio::join!(msg_handler, args).1?;
    println!("{args:?}");
    Command::new(mc.extra_data.with_java.as_ref().map_or("java", String::as_str))
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::null())
        .current_dir(mc.get_cwd())
        .spawn()?.wait().await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    real_main().await.unwrap();
}
