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

pub fn add_comment_handler(mut req: Request, res: Response, c: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);    
    let add_comment : AddComment = serde_json::from_str(&body).unwrap(); 
    let comment_body = add_comment.comment.body;
    println!("comment_body: {}", comment_body);
    
    let token =  req.headers.get::<Authorization<Bearer>>(); 
    let logged_id : i32 =  
        match token {
            Some(token) => {
                let jwt = &token.0.token;
                login(&jwt).unwrap()

            }
            _ => 0
        };

    let caps = c.unwrap();
    let slug = &caps[0].replace("/api/articles/", "").replace("/comments", "");
    println!("slug: {}", slug);

    let mut result : Option<CommentResult> = None; 

    {
        let mut sql = Core::new().unwrap();
        let follow_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
            .and_then(|conn| conn.query(                            
                "declare @id int; select top 1 @id = id from Articles where Slug = @p1 ORDER BY 1; 
                insert into Comments (createdAt, body, ArticleId ) values (getdate(), @p3, @id);
                select Comments.Id, createdAt, body,  Users.UserName, Users.Bio, Users.[Image] 
                from Comments, Users where Comments.Id = SCOPE_IDENTITY() and Users.Id = @p2
                ", &[&(slug.as_str()), &logged_id, &(comment_body.as_str()) ]
            ).for_each_row(|row| {
                let id : i32 = row.get(0);
                let created_at : NaiveDateTime = row.get(1);
                let body : &str = row.get(2);
                let user_name : &str = row.get(3);
                let bio : Option<&str> = row.get(4);
                let image : Option<&str> = row.get(5);
                let profile = Profile{ username:user_name.to_string(), bio:bio.map(|s| s.to_string()), image:image.map(|s| s.to_string()), following:false };
                let comment = Comment{ 
                    id:id, createdAt:created_at, updatedAt:created_at,
                    body:body.to_string(), author: profile
                };
                result = Some(CommentResult{comment:comment});
                Ok(())
            })
        );
        sql.run(follow_cmd).unwrap(); 
    }

    if result.is_some() {
        let result = result.unwrap();
        let result = serde_json::to_string(&result).unwrap();
        let result : &[u8] = result.as_bytes();
        res.send(&result).unwrap();                        
    }   
}


pub fn delete_comment_handler(mut req: Request, res: Response, c: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);   

    let caps = c.unwrap();
    let url_params = &caps[0];
    let slug = "aaa";
    let id = url_params.split("/").last().unwrap();
    println!("url_params: {}",url_params);
    println!("slug: {}", slug);
    println!("id: {}", id);

    let mut result : Option<Article> = None; 
    {
        let mut sql = Core::new().unwrap();
        let get_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
            .and_then(|conn| conn.query(                            
                "DELETE TOP(1) FROM Comments WHERE Id = @P1;
                SELECT 1; 
               ", &[&id]
            ).for_each_row(|row| {
                let _ : i32 = row.get(0);
                Ok(())
            })
        );
        sql.run(get_cmd).unwrap(); 
    }

    if result.is_some() {
        let result = result.unwrap();
        let result = serde_json::to_string(&result).unwrap();
        let result : &[u8] = result.as_bytes();
        res.send(&result).unwrap();                        
    }   
}

pub fn get_comments_handler(mut req: Request, res: Response, c: Captures) {    
    let token =  req.headers.get::<Authorization<Bearer>>(); 
    let logged_id : i32 =  
        match token {
            Some(token) => {
                let jwt = &token.0.token;
                login(&jwt).unwrap()

            }
            _ => 0
        };

    let caps = c.unwrap();
    let slug = &caps[0].replace("/api/articles/", "").replace("/comments", "");
    println!("slug: {}", slug);

    let mut result : Option<CommentsResult> = None; 
    let mut comments : Vec<Comment>  = Vec::new();

    {
        let mut sql = Core::new().unwrap();
        let follow_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
            .and_then(|conn| conn.query(                            
                "declare @id int; select top 1 @id = id from Articles where Slug = @p1 ORDER BY 1; 
                select Comments.Id, createdAt, body,  Users.UserName, Users.Bio, Users.[Image] 
                from Comments inner join Users ON Users.Id = Comments.Author where ArticleId = @id
                ", &[&(slug.as_str()), &logged_id ]
            ).for_each_row(|row| {
                let id : i32 = row.get(0);
                let created_at : NaiveDateTime = row.get(1);
                let body : &str = row.get(2);
                let user_name : &str = row.get(3);
                let bio : Option<&str> = row.get(4);
                let image : Option<&str> = row.get(5);
                let profile = Profile{ username:user_name.to_string(), bio:bio.map(|s| s.to_string()), image:image.map(|s| s.to_string()), following:false };
                let comment = Comment{ 
                    id:id, createdAt:created_at, updatedAt:created_at,
                    body:body.to_string(), author: profile
                };
                comments.push(comment);
                Ok(())
            })
        );
        sql.run(follow_cmd).unwrap(); 
    }
    	
    result = Some(CommentsResult{comments:comments});

    if result.is_some() {
        let result = result.unwrap();
        let result = serde_json::to_string(&result).unwrap();
        let result : &[u8] = result.as_bytes();
        res.send(&result).unwrap();                        
    }   
}

#[cfg(test)]
#[test]
fn add_comment_test() {
    let client = Client::new();

    let res = client.post("http://localhost:6767/api/users/login")
        .body(r#"{"user":{"email": "jake@jake.jake","password": "jakejake"}}"#)
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
    let token = res.headers.get::<Authorization<Bearer>>().unwrap(); 
    let jwt = &token.0.token;

    let res = client.post("http://localhost:6767/api/articles/how-to-train-your-dragon/comments")
        .header(Authorization(Bearer {token: jwt.to_owned()}))
        .body(r#"{"comment": {"body": "His name was my name too."}}"#)
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn delete_comment_test() {
    let client = Client::new();

    let res = client.post("http://localhost:6767/api/users/login")
        .body(r#"{"user":{"email": "jake@jake.jake","password": "jakejake"}}"#)
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
    let token = res.headers.get::<Authorization<Bearer>>().unwrap(); 
    let jwt = &token.0.token;

    let mut res = client.post("http://localhost:6767/api/articles/how-to-train-your-dragon/comments")
        .header(Authorization(Bearer {token: jwt.to_owned()}))
        .body(r#"{"comment": {"body": "His name was my name too."}}"#)
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 
    assert_eq!(res.status, hyper::Ok);

    println!("Got result:{:?}", buffer);
    let comment_result : CommentResult = serde_json::from_str(&buffer).unwrap();
    println!("Comment result:{:?}", comment_result);

    let mut res = client.delete(&(format!("http://localhost:6767/api/articles/how-to-train-your-dragon/comments/{}", comment_result.comment.id)))
        .header(Authorization(Bearer {token: jwt.to_owned()}))
        .body(r#"{"comment": {"body": "His name was my name too."}}"#)
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 
    assert_eq!(res.status, hyper::Ok);
}

