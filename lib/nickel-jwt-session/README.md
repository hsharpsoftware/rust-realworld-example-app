# nickel-jwt-session

Experimental jwt-based user session for nickel.
Suggestions for improvements are welcome.

[![Build Status](https://travis-ci.org/kaj/nickel-jwt-session.svg?branch=master)]
(https://travis-ci.org/kaj/nickel-jwt-session)
[![Crate](https://meritbadge.herokuapp.com/nickel-jwt-session)](https://crates.io/crates/nickel-jwt-session)

## Configuration

By default, nickel-jwt-session will store and look for the token in a cookie named "jwt", and the token will expire in 24 hours. The only required argument to the constructor is a private signing key:

```rust
extern crate nickel;
extern crate nickel_jwt_session;

use nickel::Nickel;
use nickel_jwt_session::SessionMiddleware;

fn main() {
    let mut server = Nickel::new();
    server.utilize(SessionMiddleware::new("My very secret key"));
}
```

You can also customize the cookie name:

```rust
extern crate nickel;
extern crate nickel_jwt_session;

use nickel::Nickel;
use nickel_jwt_session::{SessionMiddleware, TokenLocation};

fn main() {
    let mut server = Nickel::new();
    server.utilize(SessionMiddleware::new("My very secret key")
                   .using(TokenLocation::Cookie("my-jwt-cookie".to_owned())));
}
```

Or use Authorization: Bearer headers instead of cookies:

```rust
extern crate nickel;
extern crate nickel_jwt_session;

use nickel::Nickel;
use nickel_jwt_session::{SessionMiddleware, TokenLocation};

fn main() {
    let mut server = Nickel::new();
    server.utilize(SessionMiddleware::new("My very secret key")
                   .using(TokenLocation::AuthorizationHeader));
}
```

And change the number of seconds the token will be valid for:

```rust
extern crate nickel;
extern crate nickel_jwt_session;

use nickel::Nickel;
use nickel_jwt_session::SessionMiddleware;

fn main() {
    let mut server = Nickel::new();
    server.utilize(SessionMiddleware::new("My very secret key")
                   .expiration_time(60 * 30)); // 30 min
}
```

## Usage

### Username only

If you only want to store a username, you can use the `set_jwt_user()`, `clear_jwt()`, and `authorized_user()` convenience methods.

When you have a user that you have authenticated, use the `set_jwt_user()` method to put a new token for that user into the response:

```rust
fn login<'mw>(req: &mut Request, mut res: Response<'mw>)
              -> MiddlewareResult<'mw> {
    let authenticated_user = your_authentication_method(req);
    match authenticated_user {
        Some(username) => {
            res.set_jwt_user(username);
            res.redirect("/")
        }
        None => {
            res.redirect("/login")
        }
    }
}
```

To check to see if you have an authenticated user, use the `authorized_user()` method:

```rust
fn private<'mw>(req: &mut Request, res: Response<'mw>)
                -> MiddlewareResult<'mw> {
    match req.authorized_user() {
        Some(user) => {
            // Whatever an authorized user is allowed to do
        },
        None => res.error(StatusCode::Forbidden, "Permission denied"),
    }
}
```

And to log a user out, call the `clear_jwt()` method:

```rust
fn logout<'mw>(_req: &mut Request, mut res: Response<'mw>)
               -> MiddlewareResult<'mw> {
    res.clear_jwt();
    res.redirect("/")
}
```

### Customized claims payload only

If you would like to store arbitrary data in the claims payload instead of a username, use the `set_jwt_custom_claims()` and `valid_custom_claims()` methods. The custom claims must be in a `BTreeMap<String, Json>`. Logging out is still done with the `clear_jwt()` method.

When you have successfully authenticated, use the `set_jwt_custom_claims()` method to put a new token with the data you include into the response:

```rust
use std::collections::BTreeMap;

fn login<'mw>(req: &mut Request, mut res: Response<'mw>)
              -> MiddlewareResult<'mw> {
    let authentication_data = your_authentication_method(req);
    match authentication_data {
        Some(data) => {
            let mut d = BTreeMap::new();
            d.insert("who".to_owned(), data.who);
            d.insert("admin".to_owned(), data.admin);
            res.set_jwt_custom_claims(d);
            res.redirect("/")
        }
        None => {
            res.redirect("/login")
        }
    }
}
```

To get the claims out if the token is valid, use the `valid_custom_claims()` method:

```rust
fn private<'mw>(req: &mut Request, res: Response<'mw>)
                -> MiddlewareResult<'mw> {
    match req.valid_custom_claims() {
        Some(data) => {
            // Whatever you do with valid data in the claims
        },
        None => res.error(StatusCode::Forbidden, "Permission denied"),
    }
}
```

And to end a session, call the `clear_jwt()` method:

```rust
fn logout<'mw>(_req: &mut Request, mut res: Response<'mw>)
               -> MiddlewareResult<'mw> {
    res.clear_jwt();
    res.redirect("/")
}
```

### Username and customized claims payload

If you would like to store *both* a username and arbitrary claims data, use the `set_jwt_user_and_custom_claims()` method to add the username and data to the token. You may then use both the `authorized_user()` and `valid_custom_claims()` methods, to get the username and the claims data, respectively. For example:

```rust
use std::collections::BTreeMap;

fn login<'mw>(req: &mut Request, mut res: Response<'mw>)
              -> MiddlewareResult<'mw> {
    let authentication_data = your_authentication_method(req);
    match authentication_data {
        Some(data) => {
            let mut d = BTreeMap::new();
            d.insert("full_name".to_owned(), data.full_name);
            d.insert("admin".to_owned(), data.admin);
            res.set_jwt_user_and_custom_claims(data.username, d);
            res.redirect("/")
        }
        None => {
            res.redirect("/login")
        }
    }
}
```

To get the username and claims out if the token is valid, use the `authorized_user()` and `valid_custom_claims()` methods:

```rust
fn private<'mw>(req: &mut Request, res: Response<'mw>)
                -> MiddlewareResult<'mw> {
    match (req.authorized_user(), req.valid_custom_claims()) {
        (Some(username), Some(data)) => {
            // Whatever you do with a username and claims data
        },
        (_, _) => res.error(StatusCode::Forbidden, "Permission denied"),
    }
}
```

And to end a session, call the `clear_jwt()` method:

```rust
fn logout<'mw>(_req: &mut Request, mut res: Response<'mw>)
               -> MiddlewareResult<'mw> {
    res.clear_jwt();
    res.redirect("/")
}
```

## Examples

Full working examples can be found in the [examples](examples) directory.
Read the [API documentation](https://docs.rs/nickel-jwt-session/).

## License

Licensed under either of

 * Apache License, Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license (http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
