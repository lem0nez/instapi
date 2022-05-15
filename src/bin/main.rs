// Copyright © 2022 Nikita Dudko. All rights reserved.
// Contacts: <nikita.dudko.95@gmail.com>
// Licensed under the MIT License.

use instafetcher::auth;

fn main() {
    let secrets = auth::Secrets {
        app_id: env!("INSTAGRAM_APP_ID")
            .parse()
            .expect("Instagram application ID must be a number"),
        app_secret: env!("INSTAGRAM_APP_SECRET"),
        oauth_uri: env!("INSTAGRAM_OAUTH_URI"),
    };
}
