// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

use std::{collections, error, io::{self, Write}};

pub struct Secrets {
    pub app_id: u64,
    pub app_secret: &'static str,
    pub oauth_uri: &'static str,
}

pub trait Token {
    fn get(&self) -> &str;
    fn user_id(&self) -> u64;
    fn expiration_date(&self) -> &chrono::DateTime<chrono::Utc>;

    fn is_expired(&self) -> bool {
        chrono::Utc::now() > *self.expiration_date()
    }
}

pub struct ShortLivedToken {
    access_token: String,
    user_id: u64,
    expiration_date: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Deserialize)]
struct ShortLivedTokenResponse {
    access_token: String,
    user_id: u64,
}

impl ShortLivedToken {
    pub fn new(secrets: &Secrets) -> Result<ShortLivedToken, Box<dyn error::Error>> {
        let auth_url = format!(
            "https://api.instagram.com/oauth/authorize?client_id={}&redirect_uri={}\
            &scope=user_profile,user_media&response_type=code",
            secrets.app_id, secrets.oauth_uri
        );

        println!("Opening the authorization page...");
        if let Err(e) = open::that(&auth_url) {
            eprintln!("Failed to open a URL: {}", e);
            println!("Follow this link manually to perform the authorization: {}", auth_url);
        }

        let mut code = String::new();
        print!("Enter the authorization code: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut code)?;

        println!("Exchanging the code for a short-lived token...");
        Self::exchange(secrets, code.trim())
    }

    fn exchange(secrets: &Secrets, code: &str) -> Result<ShortLivedToken, Box<dyn error::Error>> {
        const AVAILABILITY_HOURS: i64 = 1;

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
            .error_for_status();

        if let Err(e) = response {
            return Err(Box::new(e));
        }

        let token: ShortLivedTokenResponse = response.unwrap().json()?;
        Ok(ShortLivedToken {
            access_token: token.access_token,
            user_id: token.user_id,
            expiration_date: chrono::Utc::now() + chrono::Duration::hours(AVAILABILITY_HOURS),
        })
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

pub struct LongLivedToken {
    access_token: String,
    user_id: u64,
    expiration_date: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Deserialize)]
struct LongLivedTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: i64,
}

impl LongLivedToken {
    pub fn new(
        secrets: &Secrets,
        short_lived_token: &ShortLivedToken,
    ) -> Result<LongLivedToken, Box<dyn error::Error>> {
        let response = reqwest::blocking::get(format!(
            "https://graph.instagram.com/access_token\
            ?grant_type=ig_exchange_token&client_secret={}&access_token={}",
            secrets.app_secret, short_lived_token.get()
        ))?.error_for_status()?;

        let token: LongLivedTokenResponse = response.json()?;
        Ok(LongLivedToken {
            access_token: token.access_token,
            user_id: short_lived_token.user_id,
            expiration_date: chrono::Utc::now() + chrono::Duration::seconds(token.expires_in),
        })
    }

    pub fn refresh(&mut self) -> Result<(), Box<dyn error::Error>> {
        let response = reqwest::blocking::get(format!(
            "https://graph.instagram.com/refresh_access_token\
            ?grant_type=ig_refresh_token&access_token={}",
            self.access_token
        ))?.error_for_status()?;

        let token: LongLivedTokenResponse = response.json()?;
        self.access_token = token.access_token;
        self.expiration_date = chrono::Utc::now() + chrono::Duration::seconds(token.expires_in);
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
