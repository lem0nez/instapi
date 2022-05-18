// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

//! Provides abstractions over the
//! [Instagram Basic Display API](https://developers.facebook.com/docs/instagram-basic-display-api/).

pub mod auth;
pub mod user;

use std::{error::Error, result, str::FromStr};

const BASE_URL: &str = "https://graph.instagram.com";
/// Used in requests related to the short-lived token retrieving.
const AUTH_BASE_URL: &str = "https://api.instagram.com";
const API_VERSION: &str = "v13.0";

type Result<T> = result::Result<T, Box<dyn Error>>;

/// Converts `Option<String>` to `Option<T>` using the [parse][str::parse] method.
fn parse_opt<T, E>(opt: Option<String>) -> result::Result<Option<T>, E>
where
    T: FromStr<Err = E>,
    E: Error,
{
    Ok(match opt {
        Some(str) => Some(str.parse()?),
        None => None,
    })
}

#[cfg(test)]
mod tests {
    use std::num::ParseIntError;
    use url::Url;

    #[test]
    fn parse_opt() {
        let opt_str = Some("https://example.com".to_string());
        let opt_url: Option<Url> = super::parse_opt(opt_str).unwrap();
        assert!(opt_url.is_some());

        assert_eq!(super::parse_opt::<i32, ParseIntError>(None).unwrap(), None);
    }
}
