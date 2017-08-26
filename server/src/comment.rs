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

static COMMENT_SELECT : &'static str = r#"
  select Comments.Id, createdAt, body,  Users.UserName, Users.Bio, Users.[Image] 
  from Comments inner join Users ON Users.Id = Comments.Author where Comments.Id = @commentid
"#;

fn get_comment_from_row( row : tiberius::query::QueryRow ) -> Option<CommentResult> {
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
    let result = Some(CommentResult{comment:comment});
    result    
}    

pub fn add_comment_handler(req: Request, res: Response, c: Captures) {
    let (body, logged_id) = prepare_parameters(req);

    let add_comment : AddComment = serde_json::from_str(&body).unwrap(); 
    let comment_body : &str = &add_comment.comment.body;
    println!("comment_body: {}", comment_body);
    
    let caps = c.unwrap();
    let slug = &caps[0].replace("/api/articles/", "").replace("/comments", "");
    println!("add_comment_handler slug: '{}'", slug);

    process(
        res,
        r#"declare @id int; select top 1 @id = id from Articles where Slug = @p1 ORDER BY 1; 
          DECLARE @logged int = @P2;
          insert into Comments (createdAt, body, ArticleId, Author ) values (getdate(), @p3, @id, @logged);
          declare @commentid int = SCOPE_IDENTITY(); 
        "#, 
        COMMENT_SELECT,
        get_comment_from_row,
        &[&(slug.as_str()), &logged_id, &comment_body, ]
    );
}


pub fn delete_comment_handler(req: Request, res: Response, c: Captures) {
    let (_, logged_id) = prepare_parameters(req);   

    let caps = c.unwrap();
    let url_params = &caps[0];
    let id = url_params.split("/").last().unwrap();
    println!("delete_comment_handler url_params: {}",url_params);
    println!("id: {}", id);

    process(
        res,
        r#"DELETE TOP(1) FROM Comments WHERE Id = @P1 AND Author=@P2;
        "#, 
        "SELECT 1",
        handle_row_none,
        &[&id, &logged_id]
    );

    return;
}

pub fn get_comments_handler(req: Request, res: Response, c: Captures) {    
    let (_, logged_id) = prepare_parameters(req);   

    let caps = c.unwrap();
    let slug = &caps[0].replace("/api/articles/", "").replace("/comments", "");
    println!("get_comments_handler slug: '{}'", slug);

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
use rand::Rng;

#[cfg(test)]
#[test]
fn add_comment_test() {
    let client = Client::new();

    let (jwt, slug, user_name) = login_create_article();
    let url = format!("http://localhost:6767/api/articles/{}/comments", slug);

    let comment_body = format!("His name was my name too {}-{}.", since_the_epoch(), rand::thread_rng().gen_range(0, 1000));
    let body = format!(r#"{{"comment": {{"body": "{}" }}}}"#, comment_body); 

    let mut res = client.post(&url)
        .header(Authorization(Bearer {token: jwt.to_owned()}))
        .body(&body)
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 

    let create_result : CommentResult = serde_json::from_str(&buffer).unwrap();   
    let comment = create_result.comment;  
    assert_eq!(comment.body, comment_body); 
    assert_eq!(comment.author.username,user_name);

    assert_eq!(res.status, hyper::Ok);

    let mut res = client.get(&url)
        .header(Authorization(Bearer {token: jwt}))
        .body(&body)
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 
    
    let comments : CommentsResult = serde_json::from_str(&buffer).unwrap(); 
    assert_eq!(comments.comments.len(), 1); 
}

#[cfg(test)]
#[test]
fn delete_comment_test() {
    let client = Client::new();

    let (jwt, slug, user_name) = login_create_article();
    let url = format!("http://localhost:6767/api/articles/{}/comments", slug);

    let mut res = client.post(&url)
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

    let url2 = format!("http://localhost:6767/api/articles/{}/comments/{}", slug, comment_result.comment.id);

    let mut res = client.delete(&url2)
        .header(Authorization(Bearer {token: jwt}))
        .body(r#"{"comment": {"body": "His name was my name too."}}"#)
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 
    assert_eq!(res.status, hyper::Ok);
}

