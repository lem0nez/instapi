// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

//! Authorization related stuff: tokens and application secrets.

use std::{collections, io::{self, Write}};

/// Private information that specific for an Instagram application.
pub struct Secrets {
    /// Application ID.
    pub app_id: u64,
    /// Application secret.
    pub app_secret: &'static str,
    /// Redirect URI that used upon the successful authorization.
    pub oauth_uri: &'static str,
}

/// Represents an User Access Token.
pub trait Token {
    /// Returns the user's app-scoped token.
    fn get(&self) -> &str;
    /// Get the user ID that a token belongs to.
    fn user_id(&self) -> u64;
    /// Returns the date after which a token won't be valid.
    fn expiration_date(&self) -> &chrono::DateTime<chrono::Utc>;

    /// Checks if a token isn't expired.
    fn is_valid(&self) -> bool {
        chrono::Utc::now() < *self.expiration_date()
    }
}

/// Serializable short-lived token, valid for 1 hour after retrieving.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ShortLivedToken {
    access_token: String,
    user_id: u64,
    #[serde(with = "chrono::serde::ts_seconds")]
    expiration_date: chrono::DateTime<chrono::Utc>,
}

/// Serializable long-lived token that valid for 60 days, or 90 days for private accounts.
/// Can be refreshed.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct LongLivedToken {
    access_token: String,
    user_id: u64,
    #[serde(with = "chrono::serde::ts_seconds")]
    expiration_date: chrono::DateTime<chrono::Utc>,
}

/// Abstractions over JSON responses.
mod response {
    #[derive(serde::Deserialize)]
    pub(super) struct ShortLivedToken {
        pub(super) access_token: String,
        pub(super) user_id: u64,
    }

    #[derive(serde::Deserialize)]
    pub(super) struct LongLivedToken {
        pub(super) access_token: String,
        /// Always contains the “bearer” value.
        pub(super) token_type: String,
        /// Represented in seconds.
        pub(super) expires_in: u32,
    }
}

impl ShortLivedToken {
    /// Constructs a new instance by exchanging `code` for a short-lived Instagram User Access
    /// Token. `code` can be retrieved using the [request_code] function.
    ///
    /// # Panics
    /// If a [Client][reqwest::blocking::Client] can't be initialized.
    pub fn new(secrets: &Secrets, code: &str) -> reqwest::Result<Self> {
        let app_id = secrets.app_id.to_string();
        let params: collections::HashMap<&str, &str> = [
            ("client_id", app_id.as_str()),
            ("client_secret", secrets.app_secret),
            ("redirect_uri", secrets.oauth_uri),
            ("grant_type", "authorization_code"),
            ("code", code),
        ].iter().cloned().collect();

        let client = reqwest::blocking::Client::new();
        let response = client
            .post("https://api.instagram.com/oauth/access_token")
            .form(&params)
            .send()?
            .error_for_status()?;
        Ok(response.json::<response::ShortLivedToken>()?.into())
    }
}

impl Token for ShortLivedToken {
    fn get(&self) -> &str {
        &self.access_token
    }
    fn user_id(&self) -> u64 {
        self.user_id
    }
    fn expiration_date(&self) -> &chrono::DateTime<chrono::Utc> {
        &self.expiration_date
    }
}

impl From<response::ShortLivedToken> for ShortLivedToken {
    fn from(response: response::ShortLivedToken) -> Self {
        const AVAILABILITY_HOURS: i64 = 1;
        Self {
            access_token: response.access_token,
            user_id: response.user_id,
            expiration_date: chrono::Utc::now() + chrono::Duration::hours(AVAILABILITY_HOURS),
        }
    }
}

impl LongLivedToken {
    /// Constructs a long-lived User Access Token by exchanging a short-lived token.
    /// `short_lived_token` must be valid.
    ///
    /// # Panics
    /// If `format!` panics while formatting an URL.
    pub fn new(secrets: &Secrets, short_lived_token: &ShortLivedToken) -> crate::Result<Self> {
        if !short_lived_token.is_valid() {
            return Err("short-lived token is expired".into());
        }

        let response = reqwest::blocking::get(format!(
            "https://graph.instagram.com/access_token\
            ?grant_type=ig_exchange_token&client_secret={}&access_token={}",
            secrets.app_secret, short_lived_token.get()
        ))?.error_for_status()?;

        let token: response::LongLivedToken = response.json()?;
        Ok(Self {
            access_token: token.access_token,
            user_id: short_lived_token.user_id,
            expiration_date:
                chrono::Utc::now() + chrono::Duration::seconds(token.expires_in.into()),
        })
    }

    /// Refreshes a valid token.
    ///
    /// # Panics
    /// If `format!` panics while formatting an URL.
    pub fn refresh(&mut self) -> crate::Result<()> {
        if !self.is_valid() {
            return Err("token is expired".into());
        }

        let response = reqwest::blocking::get(format!(
            "https://graph.instagram.com/refresh_access_token\
            ?grant_type=ig_refresh_token&access_token={}",
            self.access_token
        ))?.error_for_status()?;

        let token: response::LongLivedToken = response.json()?;
        self.access_token = token.access_token;
        self.expiration_date =
            chrono::Utc::now() + chrono::Duration::seconds(token.expires_in.into());
        Ok(())
    }
}

impl Token for LongLivedToken {
    fn get(&self) -> &str {
        &self.access_token
    }
    fn user_id(&self) -> u64 {
        self.user_id
    }
    fn expiration_date(&self) -> &chrono::DateTime<chrono::Utc> {
        &self.expiration_date
    }
}

/// Interactively forwards the user to the authorization page and requests a code.
/// Returns trimmed authorization code.
///
/// # Panics
/// 1. If [auth_url] panics or if failed to write to the standard output.
pub fn request_code(secrets: &Secrets) -> crate::Result<String> {
    let auth_url = auth_url(secrets)?;

    println!("Opening the authorization page...");
    if let Err(e) = open::that(auth_url.as_str()) {
        eprintln!("Failed to open an URL: {}", e);
        println!("Follow this link manually to perform the authorization: {}", auth_url);
    }

    let mut code = String::new();
    loop {
        print!("Enter the authorization code: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut code)?;

        code = code.trim().to_string();
        if !code.is_empty() {
            break;
        }
        eprintln!("You must enter a code!");
    }
    Ok(code)
}

/// Returns an URL that refers to the Authorization Window.
///
/// # Panics
/// If `format!` panics.
pub fn auth_url(secrets: &Secrets) -> Result<url::Url, url::ParseError> {
    url::Url::parse(format!(
        "https://api.instagram.com/oauth/authorize?client_id={}&redirect_uri={}\
        &scope=user_profile,user_media&response_type=code",
        secrets.app_id, secrets.oauth_uri
    ).as_str())
}

#[cfg(test)]
mod tests {
    #[test]
    fn auth_url() {
        let secrets = super::Secrets {
            app_id: 0,
            app_secret: "",
            oauth_uri: "",
        };
        assert!(super::auth_url(&secrets).is_ok())
    }
}
