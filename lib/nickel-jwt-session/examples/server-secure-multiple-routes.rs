//! A simple example server with login and session.
#[macro_use]
extern crate nickel;
extern crate nickel_jwt_session;
extern crate cookie;
extern crate hyper;
extern crate env_logger;
extern crate rustc_serialize;

use nickel::{HttpRouter, Middleware, MiddlewareResult, Nickel, NickelError,
             Request, Response, Router};
use nickel::status::StatusCode;
use nickel_jwt_session::{SessionMiddleware, SessionRequestExtensions,
                         SessionResponseExtensions, TokenLocation};
use std::collections::{BTreeMap, HashMap};
use nickel::extensions::Redirect;
use rustc_serialize::json::ToJson;

fn main() {
    env_logger::init().unwrap();
    let mut server = Nickel::new();
    server.utilize(SessionMiddleware::new("My very secret key")
                   .expiration_time(60)// Short, to see expiration.
                   .using(TokenLocation::AuthorizationHeader));

    server.get("/", public);
    server.get("/login", login);
    server.get("/logout", logout);

    let mut router = Router::new();
    router.get("/private", private);
    // Add more routes that always require authorization here.

    let authorization_required = AuthorizationRequired::new(router);
    server.utilize(authorization_required);

    server.listen("127.0.0.1:6767").expect("listen");
}

fn public<'mw>(req: &mut Request, res: Response<'mw>) -> MiddlewareResult<'mw> {
    let mut data = HashMap::new();
    data.insert("who", req.authorized_user().unwrap_or("world".to_owned()));
    res.render("examples/templates/public.tpl", &data)
}

fn login<'mw>(_req: &mut Request,
              mut res: Response<'mw>)
              -> MiddlewareResult<'mw> {
    // A real login view would get a username/password pair or a CAS
    // ticket or something, but in this example, we just consider
    // "carl" logged in.
    let mut d = BTreeMap::new();
    d.insert("full_name".to_owned(), "Carl Smith".to_json());
    d.insert("admin".to_owned(), true.to_json());
    res.set_jwt_user_and_custom_claims("carl", d);
    res.redirect("/")
}

fn logout<'mw>(_req: &mut Request,
               mut res: Response<'mw>)
               -> MiddlewareResult<'mw> {
    res.clear_jwt();
    res.redirect("/")
}

fn private<'mw>(req: &mut Request,
                res: Response<'mw>)
                -> MiddlewareResult<'mw> {
    res.render("examples/templates/private.tpl",
               &req.valid_custom_claims().expect("Already validated claims \
                                                  in AuthorizationRequired \
                                                  middleware"))
}

pub struct AuthorizationRequired<M> {
    next: M,
}

impl<M> AuthorizationRequired<M> {
    pub fn new(middleware: M) -> AuthorizationRequired<M> {
        AuthorizationRequired { next: middleware }
    }
}

impl<D, M: Middleware<D>> Middleware<D> for AuthorizationRequired<M> {
    fn invoke<'mw, 'conn>(&'mw self,
                          req: &mut Request<'mw, 'conn, D>,
                          res: Response<'mw, D>)
                          -> MiddlewareResult<'mw, D> {
        // Implement your application's authentication requirements here.
        // Could involve `authorized_user()`, `valid_custom_claims()`, or
        // one of those plus other validation as needed.
        if req.valid_custom_claims().is_none() {
            return Err(NickelError::new(res,
                                        "Permission denied",
                                        StatusCode::Forbidden));
        }

        self.next.invoke(req, res)
    }
}
