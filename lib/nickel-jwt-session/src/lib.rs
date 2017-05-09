//! An experimental middleware for jwt-based login for nickel.
//!
//! When the `SessionMiddleware` is invoked, it checks if there is a "jwt"
//! cookie or `Authorization: Bearer` header, depending on configuration.
//! If it finds a valid, properly signed jwt token, data from
//! the token is added to the request.
//!
//! Basic usage supports setting and clearing a username with the
//! `set_jwt_user()` and `clear_jwt()` methods on
//! `SessionResponseExtensions`, and accessing an authorized user's username
//! through the `SessionRequestExtensions` method `authorized_user()`.
//!
//! If, instead of a username, you would like to store arbitrary data in the
//! jwt claims payload, use the `set_jwt_custom_claims()` and
//! `clear_jwt()` methods on `SessionResponseExtensions`, and
//! access the data on a valid token using the `SessionRequestExtensions` method
//! `valid_custom_claims()`.
//!
//! Working usage examples exist in [the examples directory]
//! (https://github.com/kaj/nickel-jwt-session/tree/master/examples).

extern crate nickel;
extern crate plugin;
extern crate typemap;
extern crate jwt;
extern crate crypto;
extern crate cookie;
extern crate hyper;
#[macro_use]
extern crate log;
extern crate rustc_serialize;

use cookie::Cookie as CookiePair;
use crypto::sha2::Sha256;
use hyper::header::{self, Authorization, Bearer, SetCookie};
use jwt::{Claims, Header, Registered, Token};
use nickel::{Continue, Middleware, MiddlewareResult, Request, Response};
use plugin::Extensible;
use rustc_serialize::json::Json;
use std::collections::BTreeMap;
use std::default::Default;
use typemap::Key;

/// The middleware itself.
#[derive(Clone)]
pub struct SessionMiddleware {
    /// The key for signing jwts.  Should be kept private, but needs
    /// to be the same on multiple servers sharing a jwt domain.
    server_key: String,
    /// Value for the iss (issuer) jwt claim.
    issuer: Option<String>,
    /// How long a token should be valid after creation, in seconds
    expiration_time: u64,
    /// Where to put the token to be returned
    location: TokenLocation,
}

/// Places the token could be located.
#[derive(Clone)]
pub enum TokenLocation {
    Cookie(String),
    AuthorizationHeader,
}

impl SessionMiddleware {
    /// Create a new instance.
    ///
    /// The `server_key` is used for signing and validating the jwt token.
    pub fn new(server_key: &str) -> SessionMiddleware {
        SessionMiddleware {
            server_key: server_key.to_owned(),
            issuer: None,
            expiration_time: 24 * 60 * 60,
            location: TokenLocation::Cookie("jwt".to_owned()),
        }
    }

    /// Set a value for the iss (issuer) jwt claim.
    ///
    /// The default is to not set an issuer.
    pub fn issuer(mut self, issuer: &str) -> Self {
        self.issuer = Some(issuer.to_owned());
        self
    }

    /// Set how long a token should be valid after creation (in seconds).
    ///
    /// The default is 24 hours.
    pub fn expiration_time(mut self, expiration_time: u64) -> Self {
        self.expiration_time = expiration_time;
        self
    }

    /// Set where the token should be stored.
    ///
    /// The default is `TokenLocation::Cookie("jwt")`. Alternatively,
    /// the token can be set in the `Authorization: Bearer` header.
    pub fn using(mut self, location: TokenLocation) -> Self {
        self.location = location;
        self
    }

    fn make_token(&self,
                  user: Option<&str>,
                  custom_claims: Option<BTreeMap<String, Json>>)
                  -> Option<String> {
        let header: Header = Default::default();
        let now = current_numeric_date();
        let claims = Claims {
            reg: Registered {
                iss: self.issuer.clone(),
                sub: user.map(Into::into),
                exp: Some(now + self.expiration_time),
                nbf: Some(now),
                ..Default::default()
            },
            private: custom_claims.unwrap_or(BTreeMap::new()),
        };
        let token = Token::new(header, claims);
        token.signed(self.server_key.as_ref(), Sha256::new()).ok()
    }
}

#[derive(Debug)]
struct Session {
    authorized_user: String,
}

#[derive(Debug)]
struct CustomSession {
    claims: BTreeMap<String, Json>,
}

impl Key for SessionMiddleware {
    type Value = SessionMiddleware;
}
impl Key for Session {
    type Value = Session;
}
impl Key for CustomSession {
    type Value = CustomSession;
}

fn get_cookie<'mw, 'conn, D>(req: &Request<'mw, 'conn, D>,
                             name: &str)
                             -> Option<String> {
    if let Some(cookies) = req.origin.headers.get::<header::Cookie>() {
        for cookie in cookies.iter() {
            if cookie.name == name {
                return Some(cookie.value.clone());
            }
        }
    }
    None
}

impl<D> Middleware<D> for SessionMiddleware {
    fn invoke<'mw, 'conn>(&self,
                          req: &mut Request<'mw, 'conn, D>,
                          mut res: Response<'mw, D>)
                          -> MiddlewareResult<'mw, D> {
        res.extensions_mut().insert::<SessionMiddleware>((*self).clone());

        let jwtstr = match self.location {
            TokenLocation::Cookie(ref name) => get_cookie(req, name),
            TokenLocation::AuthorizationHeader => {
                req.origin
                    .headers
                    .get::<header::Authorization<header::Bearer>>()
                    .map(|b| b.token.clone())
            }
        };

        if let Some(jwtstr) = jwtstr {
            match Token::<Header, Claims>::parse(&jwtstr) {
                Ok(token) => {
                    if token.verify(self.server_key.as_ref(), Sha256::new()) {
                        debug!("Verified token for: {:?}", token.claims);
                        let now = current_numeric_date();
                        if let Some(nbf) = token.claims.reg.nbf {
                            if now < nbf {
                                warn!("Got a not-yet valid token: {:?}",
                                      token.claims);
                                return Ok(Continue(res));
                            }
                        }
                        if let Some(exp) = token.claims.reg.exp {
                            if now > exp {
                                warn!("Got an expired token: {:?}",
                                      token.claims);
                                return Ok(Continue(res));
                            }
                        }
                        if let Some(user) = token.claims.reg.sub {
                            info!("User {:?} is authorized for {} on {}",
                                  user,
                                  req.origin.remote_addr,
                                  req.origin.uri);
                            req.extensions_mut()
                                .insert::<Session>(Session {
                                    authorized_user: user,
                                });
                        }
                        let custom_claims = token.claims.private;
                        if !custom_claims.is_empty() {
                            info!("Custom claims {:?} are valid for {} on {}",
                                  custom_claims,
                                  req.origin.remote_addr,
                                  req.origin.uri);
                            req.extensions_mut()
                                .insert::<CustomSession>(CustomSession {
                                    claims: custom_claims,
                                });
                        }
                    } else {
                        info!("Invalid token {:?}", token);
                    }
                }
                Err(err) => {
                    info!("Bad jwt token: {:?}", err);
                }
            }
        }

        Ok(Continue(res))
    }
}

/// Extension trait for the request.
///
/// This trait is implemented for `nickel::Request`.
/// Use this trait to be able to get the token info for a nickel
/// request.
pub trait SessionRequestExtensions {
    /// Check if there is a valid token with an authorized user.
    ///
    /// If there is a valid token that has a username, Some(username)
    /// is returned, otherwise, None is returned.
    fn authorized_user(&self) -> Option<String>;

    /// Check if there is a valid token with custom claims data.
    ///
    /// If there is a valid token that has custom claims set,
    /// Some(&BTreeMap<String, Json>) is returned, otherwise, None is returned.
    fn valid_custom_claims(&self) -> Option<&BTreeMap<String, Json>>;
}

/// Extension trait for the response.
///
/// This trait is implemented for `nickel::Response`.
/// A jwt cookie or an Authorization: Bearer header signed with the
/// secret key will be added to the response.
/// It is the responsibility of the caller to actually validate
/// the user (e.g. by password, or by CAS or some other mechanism)
/// before calling this method.
/// The token will be valid for the expiration_time specified on
/// the `SessionMiddleware` from the current time.
/// Use this trait to be able to set and clear a jwt token on a nickel
/// response.
pub trait SessionResponseExtensions {
    /// Set the user. Convenience method for cases with only a username and
    /// no custom claims.
    fn set_jwt_user(&mut self, user: &str);

    /// Set the custom jwt claims data. Convenience method for cases with only
    /// custom claims and without a username.
    fn set_jwt_custom_claims(&mut self, claims: BTreeMap<String, Json>);

    /// Set both the user and custom claims.
    fn set_jwt_user_and_custom_claims(&mut self,
                                      user: &str,
                                      claims: BTreeMap<String, Json>);

    /// Clear the jwt.
    ///
    /// The response will clear the jwt cookie (set it to empty with
    /// zero max_age) or Authorization: Bearer header (set it to empty).
    fn clear_jwt(&mut self);
}

impl<'a, 'b, D> SessionRequestExtensions for Request<'a, 'b, D> {
    fn authorized_user(&self) -> Option<String> {
        if let Some(session) = self.extensions().get::<Session>() {
            debug!("Got a session: {:?}", session);
            return Some(session.authorized_user.clone());
        }
        debug!("authorized_user returning None");
        None
    }

    fn valid_custom_claims(&self) -> Option<&BTreeMap<String, Json>> {
        if let Some(custom_session) = self.extensions().get::<CustomSession>() {
            debug!("Got a session with custom claims: {:?}", custom_session);
            return Some(&custom_session.claims);
        }
        debug!("valid_custom_claims returning None");
        None
    }
}

impl<'a, 'b, D> SessionResponseExtensions for Response<'a, D> {
    fn set_jwt_user(&mut self, user: &str) {
        debug!("Should set a user jwt for {}", user);
        let (location, token, expiration) = match self.extensions()
            .get::<SessionMiddleware>() {
            Some(sm) => {
                (Some(sm.location.clone()),
                 sm.make_token(Some(user), None),
                 Some(sm.expiration_time))
            }
            None => {
                warn!("No SessionMiddleware on response.  :-(");
                (None, None, None)
            }
        };

        match (location, token, expiration) {
            (Some(location), Some(token), Some(expiration)) => {
                set_jwt(self, location, token, expiration)
            }
            (_, _, _) => {}
        }
    }

    fn set_jwt_custom_claims(&mut self, claims: BTreeMap<String, Json>) {
        debug!("Should set custom claims jwt for {:?}", claims);
        let (location, token, expiration) = match self.extensions()
            .get::<SessionMiddleware>() {
            Some(sm) => {
                (Some(sm.location.clone()),
                 sm.make_token(None, Some(claims)),
                 Some(sm.expiration_time))
            }
            None => {
                warn!("No SessionMiddleware on response.  :-(");
                (None, None, None)
            }
        };

        match (location, token, expiration) {
            (Some(location), Some(token), Some(expiration)) => {
                set_jwt(self, location, token, expiration)
            }
            (_, _, _) => {}
        }
    }

    fn set_jwt_user_and_custom_claims(&mut self,
                                      user: &str,
                                      claims: BTreeMap<String, Json>) {
        debug!("Should set a user and custom claims jwt for {}, {:?}",
               user,
               claims);
        let (location, token, expiration) = match self.extensions()
            .get::<SessionMiddleware>() {
            Some(sm) => {
                (Some(sm.location.clone()),
                 sm.make_token(Some(user), Some(claims)),
                 Some(sm.expiration_time))
            }
            None => {
                warn!("No SessionMiddleware on response.  :-(");
                (None, None, None)
            }
        };

        match (location, token, expiration) {
            (Some(location), Some(token), Some(expiration)) => {
                set_jwt(self, location, token, expiration)
            }
            (_, _, _) => {}
        }
    }

    fn clear_jwt(&mut self) {
        debug!("Should clear jwt");
        let location = match self.extensions().get::<SessionMiddleware>() {
            Some(sm) => Some(sm.location.clone()),
            None => None,
        };

        match location {
            Some(TokenLocation::Cookie(name)) => {
                let mut gone = CookiePair::new(name, "".to_owned());
                gone.max_age = Some(0);
                self.set(SetCookie(vec![gone]));
            }
            Some(TokenLocation::AuthorizationHeader) => {
                self.headers_mut()
                    .set(Authorization(Bearer { token: "".to_owned() }));
            }
            None => {}
        }
    }
}

/// Get the current value for jwt NumericDate.
///
/// Defined in RFC 7519 section 2 to be equivalent to POSIX.1 "Seconds
/// Since the Epoch".  The RFC allows a NumericDate to be non-integer
/// (for sub-second resolution), but the jwt crate uses u64.
fn current_numeric_date() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).ok().unwrap().as_secs()
}


/// Set the token in the specified location to be valid for the expiration
/// time specified from the current time.
fn set_jwt<'a, D>(response: &mut Response<'a, D>,
                  location: TokenLocation,
                  token: String,
                  expiration: u64) {
    match location {
        TokenLocation::Cookie(name) => {
            // Note: We should set secure to true on the cookie
            // but the example server is only http.
            let mut cookie = CookiePair::new(name, token);
            cookie.max_age = Some(expiration);
            debug!("Setting new cookie with token {}", cookie);
            response.set(SetCookie(vec![cookie]));
        }
        TokenLocation::AuthorizationHeader => {
            debug!("Setting new auth header with token {}", token);
            response.headers_mut().set(Authorization(Bearer { token: token }));
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
