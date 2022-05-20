// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

use std::{collections::HashMap, env, error::Error, fs, path::{Path, PathBuf}, process};
use instapi::{auth::{self, LongLivedToken, Secrets, ShortLivedToken, Token}, user::{AccountType, Profile}};

use clap::Parser;
use serde::{Serialize, de::DeserializeOwned};
use url::Url;

#[derive(Parser)]
#[clap(about, version, name = "instafetcher")]
struct Cli {
    /// Perform authorization and save a token
    #[clap(long, short)]
    login: bool,

    /// Display the user profile information
    #[clap(long, short)]
    info: bool,

    /// Download all user's media files, including album contents
    #[clap(long, short, value_name = "DIR")]
    media: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    if cli.login {
        execute_or_exit("Failed to log in", login);
    }
    if cli.info {
        execute_or_exit("Failed to retrieve the user profile information", info);
    }
}

fn execute_or_exit<F: Fn() -> Result<(), String>>(fail_msg_prefix: &str, func: F) {
    if let Err(msg) = func() {
        eprintln!("{}: {}", fail_msg_prefix, msg);
        process::exit(1);
    }
}

fn login() -> Result<(), String> {
    let secrets = Secrets {
        app_id: env!("INSTAGRAM_APP_ID")
            .parse()
            .expect("Instagram application ID must be an unsigned number"),
        app_secret: env!("INSTAGRAM_APP_SECRET"),
        oauth_uri: Url::parse(env!("INSTAGRAM_OAUTH_URI"))
            .expect("Instagram OAuth redirect URI isn't valid"),
    };

    let token_path = token_path();
    if token_path.exists() {
        println!("Warning: existing token will be overwritten");
    }

    let code = auth::request_code(&secrets);
    if let Err(e) = code {
        return Err(format!("couldn't request a code: {}", e));
    }

    let short_lived_token = ShortLivedToken::new(&secrets, code.unwrap().as_str());
    if let Err(e) = short_lived_token {
        return Err(format!("couldn't retrieve a short-lived token: {}", e));
    }

    let long_lived_token = LongLivedToken::new(&secrets, short_lived_token.unwrap());
    if let Err(e) = long_lived_token {
        return Err(format!("couldn't exchange a short-lived token for a long-lived one: {}", e));
    }

    if let Err(e) = save_token(long_lived_token.unwrap(), &token_path) {
        return Err(format!("couldn't save a token: {}", e));
    }

    print!("Token is saved");
    if let Some(str) = token_path.to_str() {
        print!(" to {}", str);
    }
    println!();
    Ok(())
}

fn info() -> Result<(), String> {
    let token: Result<LongLivedToken, _> = load_token(&token_path());
    if let Err(e) = token {
        return Err(format!(
            "couldn't load a token: {}.\nUse --login to retrieve a new token", e
        ));
    }

    let profile = Profile::new(token.unwrap());
    let info = profile.info();
    if let Err(e) = info {
        return Err(format!("couldn't get the user profile information: {}", e));
    }
    let info = info.unwrap();

    let account_types: HashMap<_, _> = [
        (AccountType::Business, "business"),
        (AccountType::MediaCreator, "media creator"),
        (AccountType::Personal, "personal"),
    ].iter().cloned().collect();

    println!(
        "User ID: {}\nUsername: @{}\nAccount type: {}\nMedia count: {}",
        profile.id(),
        info.username(),
        account_types.get(&info.account_type()).unwrap(),
        info.media_count(),
    );
    Ok(())
}

fn save_token<T: Token + Serialize>(token: T, path: &Path) -> Result<(), Box<dyn Error>> {
    let json = serde_json::to_string(&token)?;
    Ok(fs::write(path, json)?)
}

fn load_token<T: Token + DeserializeOwned>(path: &Path) -> Result<T, Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(content.as_str())?)
}

fn token_path() -> PathBuf {
    let file = Path::new("instafetcher-token").with_extension("json");
    if let Some(dir) = dirs::config_dir() {
        if dir.exists() || fs::create_dir_all(&dir).is_ok() {
            return dir.join(file);
        }
    }
    file
}
