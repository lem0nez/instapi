// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

//! Provides methods to retrieve user's information and media.

use crate::auth::Token;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, FixedOffset};
use threadpool::ThreadPool;
use url::Url;

/// Represents the user profile associated with the provided token.
pub struct Profile<T> {
    token: T,
}

/// Basic information about the user profile.
pub struct Info {
    username: String,
    account_type: AccountType,
    media_count: u64,
}

/// The user's account type.
#[derive(Clone, Copy, PartialEq)]
pub enum AccountType {
    Bussiness,
    MediaCreator,
    Personal,
}

/// Provides metadata about the user's media: images, videos and albums.
pub struct Media {
    id: u64,
    media_type: MediaType,
    username: String,
    caption: Option<String>,
    timestamp: DateTime<FixedOffset>,

    media_url: Url,
    permalink: Option<Url>,
    thumbnail_url: Option<Url>,
}

/// Type of a media item.
#[derive(Clone, Copy, PartialEq)]
pub enum MediaType {
    Image,
    Video,
    CarouselAlbum,
}

/// Abstractions over JSON responses.
mod response {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub(super) struct Info {
        pub(super) account_type: String,
        pub(super) media_count: u64,
        pub(super) username: String,
    }

    #[derive(Deserialize)]
    pub(super) struct MediaContainer {
        pub(super) data: Vec<Media>,
        pub(super) paging: Paging,
    }

    #[derive(Deserialize)]
    pub(super) struct Media {
        pub(super) caption: Option<String>,
        pub(super) id: String,
        pub(super) media_type: String,
        pub(super) media_url: String,
        pub(super) permalink: Option<String>,
        pub(super) thumbnail_url: Option<String>,
        pub(super) timestamp: String,
        pub(super) username: String,
    }

    #[derive(Deserialize)]
    pub(super) struct Paging {
        /// URL to the next page with media items.
        pub(super) next: Option<String>,
    }
}

impl<T: Token> Profile<T> {
    /// Constructs a new profile that associated with the provided `token`.
    /// Before calling make sure that `token` is valid.
    pub fn new(token: T) -> Profile<T> {
        Profile { token }
    }

    /// Returns the user ID.
    pub fn id(&self) -> u64 {
        self.token.user_id()
    }

    /// Retrieves basic information about the user.
    pub fn info(&self) -> crate::Result<Info> {
        let url = Url::parse_with_params(
            format!("{}/{}/{}", crate::BASE_URL, crate::API_VERSION, self.id()).as_str(),
            [
                ("access_token", self.token.get()),
                ("fields", "account_type,media_count,username"),
            ]
        )?;
        let response = reqwest::blocking::get(url)?.error_for_status()?;
        Info::from(response.json::<response::Info>()?)
    }

    /// Gathers all user's media items. Uses all logical CPU cores to parse responses.
    /// To gather album contents use [album][Profile::album] method.
    ///
    /// # Panics
    /// If [Client][reqwest::blocking::Client] failed to initialize.
    pub fn media(&self) -> crate::Result<Vec<Media>> {
        Self::collect_media(Url::parse_with_params(
            format!("{}/{}/{}/media", crate::BASE_URL, crate::API_VERSION, self.id()).as_str(),
            self.media_params(),
        )?)
    }

    /// Gathers all album contents. Works in the same way as [media][Profile::media] method.
    ///
    /// # Panics
    /// If [Client][reqwest::blocking::Client] failed to initialize.
    pub fn album(&self, parent: &Media) -> crate::Result<Vec<Media>> {
        if parent.media_type != MediaType::CarouselAlbum {
            return Err("parent must be an album".into());
        }

        Self::collect_media(Url::parse_with_params(
            format!("{}/{}/children", crate::BASE_URL, parent.id).as_str(),
            self.media_params(),
        )?)
    }

    /// Recursively retrieves media items by iterating over pages.
    ///
    /// # Panics
    /// If [Client][reqwest::blocking::Client] failed to initialize.
    fn collect_media(url: Url) -> crate::Result<Vec<Media>> {
        let mut url = Some(url);
        let client = reqwest::blocking::Client::new();
        let pool = ThreadPool::new(num_cpus::get());
        let media = Arc::new(Mutex::new(Vec::new()));

        while url.is_some() {
            let response = client.get(url.unwrap()).send()?.error_for_status()?;
            let media_container: response::MediaContainer = response.json()?;
            url = crate::parse_opt(media_container.paging.next)?;

            let tx = Arc::clone(&media);
            let data = media_container.data;
            pool.execute(move || {
                let mut media = tx.lock().unwrap();
                for response in data {
                    media.push(Media::from(response).unwrap());
                }
            });
        }

        pool.join();
        match Arc::try_unwrap(media) {
            Ok(mutex) => Ok(mutex.into_inner()?),
            Err(_) => Err("failed to consume result".into()),
        }
    }

    fn media_params(&self) -> [(&str, &str); 2] {
        [
            ("access_token", self.token.get()),
            (
                "fields",
                "caption,id,media_type,media_url,permalink,thumbnail_url,timestamp,username"
            ),
        ]
    }
}

impl Info {
    pub fn username(&self) -> &str {
        &self.username
    }
    /// Get a type of the user's account.
    pub fn account_type(&self) -> AccountType {
        self.account_type
    }
    /// Returns user's number of media.
    pub fn media_count(&self) -> u64 {
        self.media_count
    }

    fn from(response: response::Info) -> crate::Result<Self> {
        Ok(Self {
            username: response.username,
            account_type: match response.account_type.as_str() {
                "BUSINESS" => AccountType::Bussiness,
                "MEDIA_CREATOR" => AccountType::MediaCreator,
                "PERSONAL" => AccountType::Personal,
                _ => return Err("invalid account type".into()),
            },
            media_count: response.media_count,
        })
    }
}

impl Media {
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn media_type(&self) -> MediaType {
        self.media_type
    }
    /// Get media's owner username.
    pub fn username(&self) -> &str {
        &self.username
    }
    /// Returns `None` if a Media inside an album.
    pub fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }
    /// Returns publish date.
    pub fn timestamp(&self) -> &DateTime<FixedOffset> {
        &self.timestamp
    }

    pub fn media_url(&self) -> &Url {
        &self.media_url
    }
    /// Get permanent URL. Returns `None` if an item contains copyrighted
    /// material, or it has been flagged for a copyright violation.
    pub fn permalink(&self) -> Option<&Url> {
        self.permalink.as_ref()
    }
    /// Get thumbnail image URL. Only available for videos.
    pub fn thumbnail_url(&self) -> Option<&Url> {
        self.thumbnail_url.as_ref()
    }

    fn from(response: response::Media) -> crate::Result<Self> {
        Ok(Self {
            id: response.id.parse()?,
            media_type: match response.media_type.as_str() {
                "IMAGE" => MediaType::Image,
                "VIDEO" => MediaType::Video,
                "CAROUSEL_ALBUM" => MediaType::CarouselAlbum,
                _ => return Err("invalid media type".into()),
            },
            username: response.username,
            caption: response.caption,
            // parse_from_rfc3339 isn't working here.
            timestamp: DateTime::parse_from_str(&response.timestamp, "%FT%T%z")?,

            media_url: response.media_url.parse()?,
            permalink: crate::parse_opt(response.permalink)?,
            thumbnail_url: crate::parse_opt(response.thumbnail_url)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_info() {
        assert!(Info::from(default_info_response()).is_ok());
    }

    #[test]
    #[should_panic(expected = "invalid account type")]
    fn into_invalid_info() {
        let mut response = default_info_response();
        response.account_type = "UNKNOWN".to_string();
        Info::from(response).unwrap();
    }

    #[test]
    fn into_media() {
        assert!(Media::from(default_media_response()).is_ok());
    }

    #[test]
    #[should_panic(expected = "invalid media type")]
    fn into_invalid_media() {
        let mut response = default_media_response();
        response.media_type = "UNKNOWN".to_string();
        Media::from(response).unwrap();
    }

    fn default_info_response() -> response::Info {
        response::Info {
            account_type: "BUSINESS".to_string(),
            media_count: 0,
            username: String::new(),
        }
    }

    fn default_media_response() -> response::Media {
        response::Media {
            caption: None,
            id: '0'.to_string(),
            media_type: "IMAGE".to_string(),
            media_url: "test:".to_string(),
            permalink: None,
            thumbnail_url: None,
            timestamp: "1970-01-01T00:00:00+0000".to_string(),
            username: String::new(),
        }
    }
}
