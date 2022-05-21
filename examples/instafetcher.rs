// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

use std::{io, process};
use std::{
    collections::HashMap,
    error::Error,
    fs::{self, File},
    os::unix::prelude::PermissionsExt,
    path::{Path, PathBuf},
};

use instapi::{
    auth::{self, LongLivedToken, Secrets, ShortLivedToken, Token},
    user::{AccountType, Media, MediaType, Profile}
};

use chrono::{Duration, Utc};
use clap::Parser;
use threadpool::ThreadPool;
use url::Url;

#[derive(Parser)]
#[clap(about, author, version)]
#[clap(name = "instafetcher")]
#[clap(arg_required_else_help(true))]
struct Cli {
    /// Perform authorization and save a token
    #[clap(short, long)]
    log_in: bool,

    /// Print the user profile information
    #[clap(short, long)]
    info: bool,

    /// Download all user's media files, including album contents
    #[clap(short, long, value_name = "DIR")]
    media: Option<PathBuf>,
}

type ProgramResult = Result<(), String>;

fn main() {
    let cli = Cli::parse();

    if cli.log_in {
        run_or_exit(log_in);
    }
    if let Some(dir_path) = cli.media.as_deref() {
        run_or_exit(|| download_media(dir_path));
    }
    if cli.info {
        run_or_exit(print_info);
    }
}

fn run_or_exit<F: Fn() -> ProgramResult>(func: F) {
    if let Err(message) = func() {
        eprintln!("{}", message);
        process::exit(1);
    }
}

fn log_in() -> ProgramResult {
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

    if let Err(e) = save_token(&long_lived_token.unwrap(), &token_path) {
        return Err(format!("Couldn't save the token: {}", e));
    }
    Ok(())
}

fn print_info() -> ProgramResult {
    let token = load_token(&token_path());
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

fn download_media(dir: &Path) -> ProgramResult {
    validate_output_dir(dir)?;

    let token = load_token(&token_path());
    if let Err(e) = token {
        return Err(format!("Couldn't load a token: {}", e));
    }
    let profile = Profile::new(token.unwrap());

    println!("Gathering information about the user's media...");
    let media = profile.media();
    if let Err(e) = media {
        return Err(format!("Couldn't gather the information: {}", e));
    }

    let pool = ThreadPool::new(num_cpus::get());
    let dir = dir.to_path_buf();
    println!("Downloading media files...");

    for media in media.unwrap() {
        if media.media_type() == MediaType::CarouselAlbum {
            todo!();
        }

        let dir = dir.clone();
        pool.execute(move || {
            print_media_info(&media);
            if let Err(e) = download_media_file(dir, &media) {
                eprintln!("Failed to download media with ID {}: {}", media.id(), e);
            }
        });
    }

    pool.join();
    Ok(())
}

fn validate_output_dir(path: &Path) -> ProgramResult {
    if path.exists() {
        if path.is_file() {
            return Err("You must specify a directory path, not a file".into());
        }
        match path.read_dir() {
            Ok(mut contents) => if contents.next().is_some() {
                return Err("An existing directory must be empty".into());
            },
            Err(e) => return Err(format!("Unable to read the provided directory: {}", e)),
        }
    }

    print!("Creating ");
    if let Some(str) = path.to_str() {
        print!("directory {}", str);
    } else {
        print!("the provided directory");
    }
    println!("...");

    if let Err(e) = fs::create_dir(path) {
        return Err(format!("Failed to create the directory: {}", e));
    }
    Ok(())
}

fn print_media_info(media: &Media) {
    let types: HashMap<_, _> = [
        (MediaType::Image, "image"),
        (MediaType::Video, "video"),
        (MediaType::CarouselAlbum, "album"),
    ].iter().cloned().collect();

    // Using a buffer to print the whole message at once,
    // because this function called from multiple threads.
    let mut buffer = format!(
        "\nID: {}\nType: {}\nOwner: @{}\nPublish date: {}",
        media.id(),
        types.get(&media.media_type()).unwrap(),
        media.username(),
        media.timestamp().to_rfc2822(),
    );
    if let Some(caption) = media.caption() {
        buffer.push_str("\nCaption: ");
        buffer.push_str(caption);
    }
    println!("{}", buffer);
}

fn download_media_file(output_dir: PathBuf, media: &Media) -> Result<PathBuf, Box<dyn Error>> {
    let url = media.media_url();

    let mut filename = media.id().to_string();
    if let Some(os_extension) = Path::new(url.path()).extension() {
        if let Some(extension) = os_extension.to_str() {
            filename.push('.');
            filename.push_str(extension);
        }
    }

    let filepath = output_dir.join(filename);
    let mut file = File::create(&filepath)?;

    let response = reqwest::blocking::get(url.clone())?.error_for_status()?;
    let mut content = io::Cursor::new(response.bytes()?);
    io::copy(&mut content, &mut file)?;

    Ok(filepath)
}

fn save_token(token: &LongLivedToken, path: &Path) -> Result<(), Box<dyn Error>> {
    let json = serde_json::to_string(token)?;
    fs::write(path, json)?;

    if cfg!(unix) {
        if let Ok(metadata) = fs::metadata(path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(path, perms).ok();
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

fn load_token(path: &Path) -> Result<LongLivedToken, Box<dyn Error>> {
    const REFRESH_THRESHOLD_DAYS: i64 = 7;
    const LOGIN_SUGGESTION: &str = "(use -l or --log-in to perform authorization)";

    if !path.exists() {
        let mut message = "file".to_string();
        if let Some(str) = path.to_str() {
            message.push(' ');
            message.push_str(str);
        }
        return Err(format!("{} doesn't exist {}", message, LOGIN_SUGGESTION).into());
    }

    let json = fs::read_to_string(path)?;
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
        } else if let Err(e) = save_token(&token, path) {
            eprintln!("Failed to save the refreshed token: {}", e);
        }
    }

    Ok(token)
}

fn token_path() -> PathBuf {
    let mut path = Path::new("instafetcher-token").with_extension("json");
    if let Some(dir) = dirs::config_dir() {
        if dir.exists() || fs::create_dir_all(&dir).is_ok() {
            path = dir.join(path);
        }
    }
    path
}
