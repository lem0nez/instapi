// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

use crate::auth;

pub struct Profile<T> {
    token: T,
}

impl<T: auth::Token> Profile<T> {
    pub fn new(token: T) -> Profile<T> {
        Profile { token }
    }

    pub fn id(&self) -> u64 {
        self.token.user_id()
    }

    pub fn info(&self) -> reqwest::Result<UserInfo> {
        let response = reqwest::blocking::get(format!(
            "https://graph.instagram.com/{}\
            ?fields=account_type,media_count,username&access_token={}",
            self.id(), self.token.get()
        ))?.error_for_status()?;
        response.json()
    }
}

pub enum AccountType {
    Bussiness,
    MediaCreator,
    Personal,
    Unknown,
}

#[derive(serde::Deserialize)]
pub struct UserInfo {
    account_type: String,
    media_count: u64,
    username: String,
}

impl UserInfo {
    pub fn account_type(&self) -> AccountType {
        match self.account_type.as_str() {
            "BUSINESS" => AccountType::Bussiness,
            "MEDIA_CREATOR" => AccountType::MediaCreator,
            "PERSONAL" => AccountType::Personal,
            _ => AccountType::Unknown,
        }
    }

    pub fn media_count(&self) -> u64 {
        self.media_count
    }

    pub fn username(&self) -> &str {
        &self.username
    }
}
