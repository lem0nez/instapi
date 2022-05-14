/*
 * Copyright © 2022 Nikita Dudko. All rights reserved.
 * Contacts: <nikita.dudko.95@gmail.com>
 * Licensed under the MIT License.
 */

use std::{
    collections, convert, error, fmt,
    io::{self, Write},
};

pub struct Secrets {
    pub app_id: u64,
    pub app_secret: &'static str,
    pub oauth_uri: &'static str,
}

#[derive(Debug)]
pub struct AuthError {
    details: String,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl error::Error for AuthError {}

impl convert::From<io::Error> for AuthError {
    fn from(io_err: io::Error) -> Self {
        AuthError {
            details: format!("I/O error: {}", io_err),
        }
    }
}

impl AuthError {
    pub fn new(details: &str) -> AuthError {
        AuthError {
            details: details.to_string(),
        }
    }
}

trait Token {
    fn get(&self) -> &str;
}

pub struct ShortLivedToken {
    token: String,
    user_id: u64,
    expiration_date: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Deserialize)]
struct ShortLivedTokenResponse {
    access_token: String,
    user_id: u64,
}

impl Token for ShortLivedToken {
    fn get(&self) -> &str {
        &self.token
    }
}

impl ShortLivedToken {
    pub fn new(secrets: &Secrets) -> Result<ShortLivedToken, AuthError> {
        let auth_url = format!(
            "https://api.instagram.com/oauth/authorize\
            ?client_id={}\
            &redirect_uri={}\
            &scope=user_profile,user_media\
            &response_type=code",
            secrets.app_id, secrets.oauth_uri
        );

        println!("Opening the authorization page...");
        if let Err(e) = open::that(&auth_url) {
            eprintln!("Failed to open a URL: {}", e);
            println!("Follow this link manually to perform authorization: {}", auth_url);
        }

        let mut code = String::new();
        print!("Enter the authorization code: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut code)?;

        println!("Exchanging the code for a short-lived token...");
        match Self::exchange(secrets, code.trim()) {
            Ok(token) => Ok(token),
            Err(e) => Err(AuthError::new(format!("exchange error: {}", e).as_str())),
        }
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
            token: token.access_token,
            user_id: token.user_id,
            expiration_date: chrono::Utc::now() + chrono::Duration::hours(AVAILABILITY_HOURS),
        })
    }
}
