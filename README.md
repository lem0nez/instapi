# InstAPI
Provides abstractions over the
[Instagram Basic Display API](https://developers.facebook.com/docs/instagram-basic-display-api/)
to retrieve tokens and gather user's profile information and media.

## Example usage
```rust
use instapi::{auth, user};

let secrets = auth::Secrets {
    app_id: /* Instagram app ID */,
    app_secret: /* Instagram app secret */,
    oauth_uri: /* OAuth redirect URI */,
};

// Forward the user to the authorization page and interactively request a code.
let code = auth::request_code(&secrets)?;
// Exchange the authorization code for a short-lived token.
let token = auth::ShortLivedToken::new(&secrets, code.as_str())?;

// Link the token with profile.
let profile = user::Profile::new(token);
// Retrieve the user profile information and print username.
println!("Username: {}", profile.info()?.username());
```

## Modules description
- The `auth` module implements authorization related stuff: secrets and tokens.
  The `Secrets` structure used to store private information of your Instagram
  application. Tokens can be of two types: _short-lived_ and _long-lived_. The
  first one is only available for **1 hour** after retrieving and can't be
  refreshed. A long-lived token is produced by exchanging a short-lived token
  and it available for **60 days** (or **90 days** for private accounts) after
  retrieving.

- The `user` module provides methods to retrieve user's profile information and
  media, including albums content. Each profile is linked to a token.

## Instafetcher
An example utility that provides command-line interface for the library.

To build this tool you need to set `INSTAGRAM_APP_ID`, `INSTAGRAM_APP_SECRET`
and `INSTAGRAM_OAUTH_URI` environment variable with the corresponding values. To
perform authorization use `--log-in` option, that will store a long-lived token
in the system's configuration directory. After that you can use the following
main options:
- `--info`. Retrieve and display the basic profile information.
- `--media`. Download all media files to the given directory. File names have
  the following format: `<owner's username>_<media ID>_<publish date>`. For each
  album will be created a subdirectory. To exclude albums use `--no-albums`
  option.
