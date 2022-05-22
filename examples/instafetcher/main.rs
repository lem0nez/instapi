// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

mod media;
mod token;

use instapi::{
    auth::{self, LongLivedToken, Secrets, ShortLivedToken},
    user::{AccountType, Profile},
};

use std::{fs, process};
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
};
use clap::Parser;
use url::Url;

#[derive(Parser)]
#[clap(about, author, version)]
#[clap(name = env!("CARGO_CRATE_NAME"))]
#[clap(arg_required_else_help = true)]
struct Cli {
    /// Perform authorization and save a token
    #[clap(short, long)]
    log_in: bool,

    /// Print the user profile information
    #[clap(short, long)]
    info: bool,

    /// Download all user's media files
    #[clap(short, long, value_name = "DIR")]
    #[clap(forbid_empty_values = true, parse(try_from_os_str = validate_output_dir))]
    media: Option<PathBuf>,

    /// Don't download albums content
    #[clap(long)]
    no_albums: bool,
}

fn main() {
    let cli = Cli::parse();

    if cli.log_in {
        run_or_exit(log_in);
    }
    if let Some(dir) = cli.media.as_deref() {
        run_or_exit(|| media::download_all(dir, !cli.no_albums));
    }
    if cli.info {
        run_or_exit(print_info);
    }
}

/// Performs authorization, retrieves a long-lived token and saves it.
///
/// # Panics
/// If invalid secrets provided.
fn log_in() -> Result<(), String> {
    let secrets = Secrets {
        app_id: env!("INSTAGRAM_APP_ID")
            .parse()
            .expect("Instagram application ID must be an unsigned number"),
        app_secret: env!("INSTAGRAM_APP_SECRET"),
        oauth_uri: Url::parse(env!("INSTAGRAM_OAUTH_URI"))
            .expect("Instagram OAuth redirect URI isn't valid"),
    };

    let token_path = token::path();
    if token_path.exists() {
        println!("Warning: existing token will be overwritten");
    }

    let code = auth::request_code(&secrets);
    if let Err(e) = code {
        return Err(format!("Couldn't request a code: {}", e));
    }

    println!("Retrieving a short-lived token...");
    let short_lived_token = ShortLivedToken::new(&secrets, code.unwrap().as_str());
    if let Err(e) = short_lived_token {
        return Err(format!("Couldn't retrieve the token: {}", e));
    }

    println!("Exchanging the token for a long-lived one...");
    let long_lived_token = LongLivedToken::new(&secrets, short_lived_token.unwrap());
    if let Err(e) = long_lived_token {
        return Err(format!("Couldn't exchange the token: {}", e));
    }

    if let Err(e) = token::save(&long_lived_token.unwrap(), Some(token_path.as_path())) {
        return Err(format!("Couldn't save the token: {}", e));
    }
    Ok(())
}

/// Loads a token and displays the basic user information.
fn print_info() -> Result<(), String> {
    let token = token::load(None);
    if let Err(e) = token {
        return Err(format!("Couldn't load a token: {}", e));
    }
    let profile = Profile::new(token.unwrap());

    println!("Retrieving the user profile information...");
    let info = profile.info();
    if let Err(e) = info {
        return Err(format!("Couldn't retrieve the information: {}", e));
    }
    let info = info.unwrap();

    let account_types: HashMap<_, _> = [
        (AccountType::Business, "business"),
        (AccountType::MediaCreator, "media creator"),
        (AccountType::Personal, "personal"),
    ].iter().cloned().collect();

    println!(
        "\nUser ID: {}\nUsername: @{}\nAccount type: {}\nMedia count: {}",
        profile.id(),
        info.username(),
        account_types.get(&info.account_type()).unwrap(),
        info.media_count(),
    );
    Ok(())
}

/// If `func` returns `Err`, prints an error message and terminates the current process.
///
/// # Panics
/// If `func` panics or if failed to write to the standard output.
fn run_or_exit<F: Fn() -> Result<(), String>>(func: F) {
    if let Err(message) = func() {
        eprintln!("{}", message);
        process::exit(1);
    }
}

/// If a directory exists, checks if it empty and readable, otherwise creates a new one.
///
/// # Panics
/// If `format!` panics.
fn validate_output_dir(path: &OsStr) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if path.exists() {
        if path.is_file() {
            return Err("it's a file".into());
        }
        match path.read_dir() {
            Ok(mut contents) => if contents.next().is_some() {
                return Err("directory must be empty".into());
            },
            Err(e) => return Err(format!("unable to read directory ({})", e)),
        }
    } else if let Err(e) = fs::create_dir(path) {
        return Err(format!("failed to create directory ({})", e));
    }
    Ok(path.to_path_buf())
}
