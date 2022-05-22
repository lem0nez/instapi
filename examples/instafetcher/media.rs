// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

//! Functions to download media files.

use crate::token;
use instapi::{
    auth::LongLivedToken,
    user::{Media, MediaType, Profile},
};

use std::{
    collections::HashMap,
    error::Error,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};
use threadpool::ThreadPool;

/// Loads a token, gathers media information and downloads contents to `output_dir`.
///
/// # Panics
/// 1. If [token::load], [instapi::user::Profile::media], [download_album] or `format!` panics.
/// 2. If failed to write to the standard output.
pub fn download_all(output_dir: &Path, include_albums: bool) -> Result<(), String> {
    let token = token::load(None);
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
    println!("Downloading media...");
    for media in media.unwrap() {
        if media.media_type() == MediaType::CarouselAlbum {
            if include_albums {
                download_album(&media, output_dir, &profile, &pool);
            }
            continue;
        }

        let output_dir = output_dir.to_path_buf();
        pool.execute(move || {
            print(&media, None);
            if let Err(e) = download_file(&media, &output_dir) {
                eprintln!("Failed to download media with ID {}: {}", media.id(), e);
            }
        });
    }
    pool.join();
    Ok(())
}

/// Gathers album information, creates a directory and downloads album contents to it.
///
/// # Panics
/// 1. If [print], [instapi::user::Profile::album] or [filename] panics.
/// 2. If failed to write to the standard output.
fn download_album(
    album: &Media,
    output_dir: &Path,
    profile: &Profile<LongLivedToken>,
    pool: &ThreadPool
) {
    print(album, None);

    let media = profile.album(album);
    if let Err(e) = media {
        eprintln!("Couldn't gather content information of album with ID {}: {}", album.id(), e);
        return;
    }

    let output_dir = output_dir.join(filename(album));
    if let Err(e) = fs::create_dir(&output_dir) {
        eprintln!("Failed to create directory for album with ID {}: {}", album.id(), e);
        return;
    }

    let album_id = album.id();
    for media in media.unwrap() {
        let output_dir = output_dir.clone();
        pool.execute(move || {
            print(&media, Some(album_id));
            if let Err(e) = download_file(&media, &output_dir) {
                eprintln!("Failed to download album media with ID {}: {}", media.id(), e);
            }
        });
    }
}

/// Prints `media` information to the standard output. `parent_id` is ID of album the media is in.
///
/// # Panics
/// If `format!` panics or if failed to write to the output.
fn print(media: &Media, parent_id: Option<u64>) {
    let types: HashMap<_, _> = [
        (MediaType::Image, "image"),
        (MediaType::Video, "video"),
        (MediaType::CarouselAlbum, "album"),
    ].iter().cloned().collect();

    // Using a buffer to print the whole message at once,
    // because the function called from multiple threads.
    let mut buffer = format!("\nID: {}", media.id());

    if let Some(id) = parent_id {
        buffer.push_str(format!("\nParent album ID: {}", id).as_str());
    }

    buffer.push_str(format!(
        "\nType: {}\nOwner: @{}\nPublish date: {}",
        types.get(&media.media_type()).unwrap(),
        media.username(),
        media.timestamp().to_rfc2822(),
    ).as_str());

    if let Some(caption) = media.caption() {
        buffer.push_str("\nCaption: ");
        buffer.push_str(caption);
    }

    println!("{}", buffer);
}

/// Downloads `media`'s content to the `output_dir`. File name constructs using [filename].
/// Extension retrieves from URL. Return path to the downloaded file.
///
/// # Panics
/// If [filename] panics.
fn download_file(media: &Media, output_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let url = media.media_url();

    let mut filename = filename(media);
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

/// Constructs a file name based on media's metadata.
///
/// # Panics
/// If `format!` panics.
fn filename(media: &Media) -> String {
    format!(
        "{}_{}_{}",
        media.username(),
        media.id(),
        media.timestamp().format("%FT%H-%M-%S"),
    )
}
