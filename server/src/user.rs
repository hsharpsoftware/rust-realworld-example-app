extern crate bson;

extern crate iis;
extern crate hyper;

extern crate serde;
extern crate serde_json;

extern crate chrono;

extern crate crypto;

extern crate futures;
extern crate tokio_core;
extern crate tiberius;

extern crate toml;

extern crate reroute;

extern crate jwt;

extern crate futures_state_stream;

extern crate slug;

use futures::Future;
use tokio_core::reactor::Core;
use tiberius::{SqlConnection};
use tiberius::stmt::ResultStreamExt;
use tiberius::stmt::Statement;

use chrono::prelude::*;

use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::env;
use std::path::PathBuf;

use hyper::server::{Server, Request, Response};
use reroute::{RouterBuilder, Captures};
use hyper::header::{Authorization, Bearer};
use hyper::status::StatusCode;

use crypto::sha2::Sha256;

use jwt::{
    Header,
    Registered,
    Token,
};

use slug::slugify;

use super::*;

pub fn new_token(user_id: &str, _: &str) -> Option<String> {
    let header: jwt::Header = Default::default();
    let claims = jwt::Registered {
        iss: Some("mikkyang.com".into()),
        sub: Some(user_id.into()),
        ..Default::default()
    };
    let token = Token::new(header, claims);

    token.signed(b"secret_key", Sha256::new()).ok()
}

pub fn login(token: &str) -> Option<i32> {
    let token = Token::<Header, Registered>::parse(token).unwrap();

    if token.verify(b"secret_key", Sha256::new()) {
        match token.claims.sub {
            Some(token) => 
                match token.parse::<i32>() {
                    Ok(result) => Some(result),
                    Err(_) => None
                }
            ,_ => None
        }    
        
        
    } else {
        None
    }
}

pub fn registration_handler(mut req: Request, _: Response, _: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);    
    let registration : Registration = serde_json::from_str(&body).unwrap();     

    let email : &str = &registration.user.email;
    let token : &str = &crypto::pbkdf2::pbkdf2_simple(&registration.user.password, 10000).unwrap();
    let username : &str = &registration.user.username;

    let mut lp = Core::new().unwrap();
    let future = SqlConnection::connect(lp.handle(), CONNECTION_STRING.as_str())
    .and_then(|conn| {
        conn.query( "
        INSERT INTO [dbo].[Users]
            ([Email]
            ,[Token]
            ,[UserName])
        VALUES
            (@P1
            ,@P2
            ,@P3); SELECT SCOPE_IDENTITY()" , &[ &email, &token, &username]  ).for_each_row( handle_row_no_value )
    } );
     lp.run(future).unwrap();
}

#[cfg(test)]
use hyper::Client;

#[cfg(test)]
#[test]
fn registration_test() {
    let client = Client::new();

    let res = client.post("http://localhost:6767/api/users")
        .body(r#"{"user":{"username": "Jacob","email": "jake@jake.jake","password": "jakejake"}}"#)
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn login_test() {
    let client = Client::new();

    let res = client.post("http://localhost:6767/api/users/login")
        .body(r#"{"user":{"email": "jake@jake.jake","password": "jakejake"}}"#)
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}