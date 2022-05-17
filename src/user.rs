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
    media_url: url::Url,
    // Will be omitted if an item contains copyrighted material,
    // or has been flagged for a copyright violation.
    permalink: Option<url::Url>,
    // Only available for videos.
    thumbnail_url: Option<url::Url>,
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
    pub(super) struct Paging {
        pub(super) next: Option<String>,
    }

    #[derive(serde::Deserialize)]
    pub(super) struct Media {
        pub(super) data: Vec<MediaItem>,
        pub(super) paging: Paging,
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
        Info::from(response.json::<response::Info>()?)
    }

    pub fn media<F: Fn(&MediaItem)>(&self, callback: F) -> crate::Result<Vec<MediaItem>> {
        let mut request_url = format!(
            "https://graph.instagram.com/{}/{}/media?access_token={}\
            &fields=caption,id,media_type,media_url,permalink,thumbnail_url,timestamp,username",
            API_VERSION, self.id(), self.token.get()
        );

        let client = reqwest::blocking::Client::new();
        let mut items = Vec::new();

        while !request_url.is_empty() {
            let response = client.get(&request_url).send()?.error_for_status()?;
            request_url.clear();

            let media: response::Media = response.json()?;
            for response_item in media.data {
                let media_item = MediaItem::from(response_item)?;
                callback(&media_item);
                items.push(media_item);
            }
            if let Some(url) = media.paging.next {
                request_url = url;
            }
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

    fn from(response: response::Info) -> crate::Result<Self> {
        Ok(Self {
            account_type: match response.account_type.as_str() {
                "BUSINESS" => AccountType::Bussiness,
                "MEDIA_CREATOR" => AccountType::MediaCreator,
                "PERSONAL" => AccountType::Personal,
                _ => return Err("invalid account type".into()),
            },
            media_count: response.media_count,
            username: response.username,
        })
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
    pub fn media_url(&self) -> &url::Url {
        &self.media_url
    }
    pub fn permalink(&self) -> &Option<url::Url> {
        &self.permalink
    }
    pub fn thumbnail_url(&self) -> &Option<url::Url> {
        &self.thumbnail_url
    }
    pub fn timestamp(&self) -> &chrono::DateTime<chrono::offset::FixedOffset> {
        &self.timestamp
    }
    pub fn username(&self) -> &str {
        &self.username
    }

    fn from(response: response::MediaItem) -> crate::Result<Self> {
        Ok(Self {
            caption: response.caption,
            id: response.id.parse()?,
            media_type: match response.media_type.as_str() {
                "IMAGE" => MediaType::Image,
                "VIDEO" => MediaType::Video,
                "CAROUSEL_ALBUM" => MediaType::CarouselAlbum,
                _ => return Err("invalid media type".into()),
            },
            media_url: response.media_url.parse()?,
            permalink: match response.permalink {
                Some(str) => Some(str.parse()?),
                None => None,
            },
            thumbnail_url: match response.thumbnail_url {
                Some(str) => Some(str.parse()?),
                None => None,
            },
            // parse_from_rfc3339 isn't working here.
            timestamp: chrono::DateTime::parse_from_str(&response.timestamp, "%FT%T%z")?,
            username: response.username,
        })
    }
}
