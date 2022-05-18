// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

use crate::auth::Token;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, FixedOffset};
use threadpool::ThreadPool;
use url::Url;

pub struct Profile<T: Token> {
    token: T,
}

pub struct Info {
    username: String,
    account_type: AccountType,
    media_count: u64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AccountType {
    Bussiness,
    MediaCreator,
    Personal,
}

#[derive(Debug)]
pub struct MediaItem {
    id: u64,
    media_type: MediaType,
    username: String,
    caption: Option<String>,
    timestamp: DateTime<FixedOffset>,

    media_url: Url,
    permalink: Option<Url>,
    thumbnail_url: Option<Url>,
}

#[derive(Clone, Copy, PartialEq)]
#[derive(Debug)]
pub enum MediaType {
    Image,
    Video,
    CarouselAlbum,
}

mod response {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub(super) struct Info {
        pub(super) account_type: String,
        pub(super) media_count: u64,
        pub(super) username: String,
    }

    #[derive(Deserialize)]
    pub(super) struct Media {
        pub(super) data: Vec<MediaItem>,
        pub(super) paging: Paging,
    }

    #[derive(Deserialize)]
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

    #[derive(Deserialize)]
    pub(super) struct Paging {
        pub(super) next: Option<String>,
    }
}

impl<T: Token> Profile<T> {
    pub fn new(token: T) -> Profile<T> {
        Profile { token }
    }

    pub fn id(&self) -> u64 {
        self.token.user_id()
    }

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

    pub fn media(&self) -> crate::Result<Vec<MediaItem>> {
        Self::collect_media(Url::parse_with_params(
            format!("{}/{}/{}/media", crate::BASE_URL, crate::API_VERSION, self.id()).as_str(),
            self.media_params(),
        )?)
    }

    pub fn album(&self, parent: &MediaItem) -> crate::Result<Vec<MediaItem>> {
        if parent.media_type != MediaType::CarouselAlbum {
            return Err("must be an album".into());
        }

        Self::collect_media(Url::parse_with_params(
            format!("{}/{}/children", crate::BASE_URL, parent.id).as_str(),
            self.media_params(),
        )?)
    }

    fn collect_media(url: Url) -> crate::Result<Vec<MediaItem>> {
        let mut url = Some(url);
        let client = reqwest::blocking::Client::new();
        let pool = ThreadPool::new(num_cpus::get());
        let media = Arc::new(Mutex::new(Vec::new()));

        while url.is_some() {
            let response = client.get(url.unwrap()).send()?.error_for_status()?;
            let media_response: response::Media = response.json()?;
            url = crate::parse_opt(media_response.paging.next)?;

            let tx = Arc::clone(&media);
            let data = media_response.data;
            pool.execute(move || {
                let mut media = tx.lock().unwrap();
                for response in data {
                    media.push(MediaItem::from(response).unwrap());
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
            ("fields", "caption,id,media_type,media_url,permalink,thumbnail_url,timestamp,username"),
        ]
    }
}

impl Info {
    pub fn username(&self) -> &str {
        &self.username
    }
    pub fn account_type(&self) -> AccountType {
        self.account_type
    }
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

impl MediaItem {
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn media_type(&self) -> MediaType {
        self.media_type
    }
    pub fn username(&self) -> &str {
        &self.username
    }
    /// Returns `None` for an album item.
    pub fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }
    pub fn timestamp(&self) -> &DateTime<FixedOffset> {
        &self.timestamp
    }

    pub fn media_url(&self) -> &Url {
        &self.media_url
    }
    /// Returns `None` if an item contains copyrighted material,
    /// or it has been flagged for a copyright violation.
    pub fn permalink(&self) -> Option<&Url> {
        self.permalink.as_ref()
    }
    /// URL available only for videos.
    pub fn thumbnail_url(&self) -> Option<&Url> {
        self.thumbnail_url.as_ref()
    }

    fn from(response: response::MediaItem) -> crate::Result<Self> {
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
