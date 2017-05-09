//! A simple example server with login and session.
#[macro_use] extern crate nickel;
extern crate nickel_jwt_session;
extern crate cookie;
extern crate hyper;
extern crate env_logger;
extern crate rustc_serialize;

use nickel::{HttpRouter, Nickel, Request, Response, MiddlewareResult};
use nickel::status::StatusCode;
use nickel_jwt_session::{SessionMiddleware, SessionRequestExtensions, SessionResponseExtensions, TokenLocation};
use std::collections::{BTreeMap, HashMap};
use nickel::extensions::Redirect;
use rustc_serialize::json::ToJson;

fn main() {
    env_logger::init().unwrap();
    let mut server = Nickel::new();
    server.utilize(SessionMiddleware::new("My very secret key")
                   .expiration_time(60)// Short, to see expiration.
                   .using(TokenLocation::AuthorizationHeader));

    server.get("/",   public);
    server.get("/login", login);
    server.get("/logout", logout);
    server.get("/private", private);

    server.listen("127.0.0.1:6767").expect("listen");
}

fn public<'mw>(req: &mut Request, res: Response<'mw>)
               -> MiddlewareResult<'mw>  {
    let mut data = HashMap::new();
    data.insert("who", req.authorized_user().unwrap_or("world".to_owned()));
    res.render("examples/templates/public.tpl", &data)
}

fn login<'mw>(_req: &mut Request, mut res: Response<'mw>)
              -> MiddlewareResult<'mw>  {
    // A real login view would get a username/password pair or a CAS
    // ticket or something, but in this example, we just consider
    // "carl" logged in.
    let mut d = BTreeMap::new();
    d.insert("full_name".to_owned(), "Carl Smith".to_json());
    d.insert("admin".to_owned(), true.to_json());
    res.set_jwt_user_and_custom_claims("carl", d);
    res.redirect("/")
}

fn logout<'mw>(_req: &mut Request, mut res: Response<'mw>)
               -> MiddlewareResult<'mw>  {
    res.clear_jwt();
    res.redirect("/")
}

fn private<'mw>(req: &mut Request, res: Response<'mw>)
                -> MiddlewareResult<'mw>  {
    match req.valid_custom_claims() {
        Some(claims) => {
            res.render("examples/templates/private.tpl", &claims)
        }
        None => res.error(StatusCode::Forbidden, "Permission denied")
    }
}
