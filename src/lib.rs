// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

use std::{error, result};

pub mod auth;
pub mod user;

pub(crate) type Result<T> = result::Result<T, Box<dyn error::Error>>;
