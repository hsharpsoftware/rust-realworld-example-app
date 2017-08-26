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

use chrono::prelude::*;

use reroute::{Captures};

use super::*;

static COMMENT_SELECT : &'static str = r#"
  select Comments.Id, createdAt, body,  Users.UserName, Users.Bio, Users.[Image], 
  (SELECT COUNT(*) FROM Followings WHERE FollowerId=@logged AND Author=FollowingId) as [Following]
  from Comments inner join Users ON Users.Id = Comments.Author where Comments.Id = @commentid
"#;

fn get_simple_comment_from_row( row : tiberius::query::QueryRow ) -> Option<Comment> {
    let id : i32 = row.get(0);
    let created_at : NaiveDateTime = row.get(1);
    let body : &str = row.get(2);
    let user_name : &str = row.get(3);
    let bio : Option<&str> = row.get(4);
    let image : Option<&str> = row.get(5);
    let f : i32 = row.get(6);
    let following : bool = f == 1;    
    let profile = Profile{ username:user_name.to_string(), bio:bio.map(|s| s.to_string()), image:image.map(|s| s.to_string()), following:following };
    let comment = Comment{ 
        id:id, createdAt:created_at, updatedAt:created_at,
        body:body.to_string(), author: profile
    };
    Some(comment)
}


fn get_comment_from_row( row : tiberius::query::QueryRow ) -> Option<CommentResult> {
    let result = Some(CommentResult{comment:get_simple_comment_from_row(row).unwrap()});
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

fn comments_result( _ : CommentsResult ) {}

pub fn get_comments_handler(req: Request, res: Response, c: Captures) {    
    let (_, logged_id) = prepare_parameters(req);   

    let caps = c.unwrap();
    let slug = &caps[0].replace("/api/articles/", "").replace("/comments", "");
    println!("get_comments_handler slug: '{}'", slug);

    process_container(
        res,
        r#"declare @id int; select top 1 @id = id from Articles where Slug = @p1 ORDER BY 1;
        declare @logged int = @p2;
        "#,
        r#"select Comments.Id, createdAt, body,  Users.UserName, Users.Bio, Users.[Image],
        (SELECT COUNT(*) FROM Followings WHERE FollowerId=@logged AND Author=FollowingId) as [Following]
                from Comments inner join Users ON Users.Id = Comments.Author where ArticleId = @id"#,
        get_simple_comment_from_row,
        comments_result,
        &[&(slug.as_str()),&logged_id]
    );
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

    let (jwt, slug, _) = login_create_article();
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

    let mut res = client.get(&url)
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 
    
    let comments : CommentsResult = serde_json::from_str(&buffer).unwrap(); 
    assert_eq!(comments.comments.len(), 0);     
}

