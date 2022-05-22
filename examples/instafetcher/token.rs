// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

use instapi::auth::{LongLivedToken, Token};

use std::{
    error::Error,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
use chrono::{Duration, Utc};

pub fn load(path: Option<&Path>) -> Result<LongLivedToken, Box<dyn Error>> {
    const REFRESH_THRESHOLD_DAYS: i64 = 7;
    const LOGIN_SUGGESTION: &str = "(use --log-in to perform authorization)";

    let path = match path {
        Some(path) => path.to_path_buf(),
        None => self::path(),
    };
    if !path.exists() {
        let mut message = "file".to_string();
        if let Some(str) = path.to_str() {
            message.push(' ');
            message.push_str(str);
        }
        return Err(format!("{} doesn't exist {}", message, LOGIN_SUGGESTION).into());
    }

    let json = fs::read_to_string(&path)?;
    let mut token: LongLivedToken = serde_json::from_str(json.as_str())?;
    if !token.is_valid() {
        return Err(format!("token has been expired {}", LOGIN_SUGGESTION).into());
    }

    let current_date = Utc::now();
    let expiration_date = *token.expiration_date();
    if expiration_date - Duration::days(REFRESH_THRESHOLD_DAYS) < current_date {
        println!(
            "Refreshing a token as it expires in {} days...",
            (expiration_date - current_date).num_days(),
        );

        if let Err(e) = token.refresh() {
            eprintln!("Failed to refresh the token: {}", e);
        } else if let Err(e) = save(&token, Some(path.as_path())) {
            eprintln!("Failed to save the refreshed token: {}", e);
        }
    }

    Ok(token)
}

pub fn save(token: &LongLivedToken, path: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let path = match path {
        Some(path) => path.to_path_buf(),
        None => self::path(),
    };

    let json = serde_json::to_string(token)?;
    fs::write(&path, json)?;

    if cfg!(unix) {
        if let Ok(metadata) = fs::metadata(&path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms).ok();
        }
    }

    print!("Token saved");
    if let Some(str) = path.to_str() {
        print!(" to {}", str);
    }
    println!(
        " (expires in {} days if not used)",
        (*token.expiration_date() - Utc::now()).num_days()
    );
    Ok(())
}

pub fn path() -> PathBuf {
    let mut path = Path::new(
        format!("{}-token", env!("CARGO_CRATE_NAME")).as_str()
    ).with_extension("json");

    if let Some(dir) = dirs::config_dir() {
        if dir.exists() || fs::create_dir_all(&dir).is_ok() {
            path = dir.join(path);
        }
    }
    path
}
