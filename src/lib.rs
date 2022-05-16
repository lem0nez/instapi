// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

//! Provides abstractions over the
//! [Instagram Basic Display API](https://developers.facebook.com/docs/instagram-basic-display-api/).

use std::{error, result};

pub mod auth;
pub mod user;

pub(crate) type Result<T> = result::Result<T, Box<dyn error::Error>>;
