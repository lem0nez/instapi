// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

use crate::auth;

const API_VERSION: &str = "v13.0";

pub struct Profile<T> {
    token: T,
}

pub enum AccountType {
    Bussiness,
    MediaCreator,
    Personal,
}

pub struct Info {
    account_type: AccountType,
    media_count: u64,
    username: String,
}

pub enum MediaType {
    Image,
    Video,
    CarouselAlbum,
}

pub struct MediaItem {
    // Isn't present for album items.
    caption: Option<String>,
    id: u64,
    media_type: MediaType,
    media_url: String,
    // Will be omitted if an item contains copyrighted material,
    // or has been flagged for a copyright violation.
    permalink: Option<String>,
    // Only available for videos.
    thumbnail_url: Option<String>,
    timestamp: chrono::DateTime<chrono::offset::FixedOffset>,
    username: String,
}

mod response {
    #[derive(serde::Deserialize)]
    pub(super) struct Info {
        pub(super) account_type: String,
        pub(super) media_count: u64,
        pub(super) username: String,
    }

    #[derive(serde::Deserialize)]
    pub(super) struct Media {
        pub(super) data: Vec<MediaItem>,
    }

    #[derive(serde::Deserialize)]
    pub(super) struct MediaItem {
        pub(super) caption: Option<String>,
        pub(super) id: String,
        pub(super) media_type: String,
        pub(super) media_url: String,
        pub(super) permalink: Option<String>,
        pub(super) thumbnail_url: Option<String>,
        pub(super) timestamp: String,
        pub(super) username: String,
    }
}

impl<T: auth::Token> Profile<T> {
    pub fn new(token: T) -> Profile<T> {
        Profile { token }
    }

    pub fn id(&self) -> u64 {
        self.token.user_id()
    }

    pub fn info(&self) -> crate::Result<Info> {
        let response = reqwest::blocking::get(format!(
            "https://graph.instagram.com/{}/{}\
            ?fields=account_type,media_count,username&access_token={}",
            API_VERSION, self.id(), self.token.get()
        ))?.error_for_status()?;

        let info: response::Info = response.json()?;
        Ok(Info {
            account_type: match info.account_type.as_str() {
                "BUSINESS" => AccountType::Bussiness,
                "MEDIA_CREATOR" => AccountType::MediaCreator,
                "PERSONAL" => AccountType::Personal,
                _ => return Err("invalid account type".into()),
            },
            media_count: info.media_count,
            username: info.username,
        })
    }

    pub fn recent_media(&self) -> crate::Result<Vec<MediaItem>> {
        let response = reqwest::blocking::get(format!(
            "https://graph.instagram.com/{}/{}/media?access_token={}\
            &fields=caption,id,media_type,media_url,permalink,thumbnail_url,timestamp,username",
            API_VERSION, self.id(), self.token.get()
        ))?.error_for_status()?;

        let media: response::Media = response.json()?;
        let mut items = Vec::new();

        for item in media.data {
            items.push(MediaItem {
                caption: item.caption,
                id: item.id.parse()?,
                media_type: match item.media_type.as_str() {
                    "IMAGE" => MediaType::Image,
                    "VIDEO" => MediaType::Video,
                    "CAROUSEL_ALBUM" => MediaType::CarouselAlbum,
                    _ => return Err("invalid media type".into()),
                },
                media_url: item.media_url,
                permalink: item.permalink,
                thumbnail_url: item.thumbnail_url,
                // parse_from_rfc3339 isn't working here.
                timestamp: chrono::DateTime::parse_from_str(&item.timestamp, "%FT%T%z")?,
                username: item.username,
            });
        }
        Ok(items)
    }
}

impl Info {
    pub fn account_type(&self) -> &AccountType {
        &self.account_type
    }
    pub fn media_count(&self) -> u64 {
        self.media_count
    }
    pub fn username(&self) -> &str {
        &self.username
    }
}

impl MediaItem {
    pub fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }
    pub fn media_url(&self) -> &str {
        &self.media_url
    }
    pub fn permalink(&self) -> Option<&str> {
        self.permalink.as_deref()
    }
    pub fn thumbnail_url(&self) -> Option<&str> {
        self.thumbnail_url.as_deref()
    }
    pub fn timestamp(&self) -> &chrono::DateTime<chrono::offset::FixedOffset> {
        &self.timestamp
    }
    pub fn username(&self) -> &str {
        &self.username
    }
}
