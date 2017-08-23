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


pub fn create_article_handler(mut req: Request, res: Response, _: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);    
    let token =  req.headers.get::<Authorization<Bearer>>(); 
    let mut result : Option<Article> = None; 
    match token {
        Some(token) => {
            let jwt = &token.0.token;
            let logged_in_user_id = login(&jwt);  

            match logged_in_user_id {
                Some(logged_in_user_id) => {
                    println!("logged_in_user {}", &logged_in_user_id);

                    let create_article : CreateArticle = serde_json::from_str(&body).unwrap();     
                    let title : &str = &create_article.article.title;
                    let description : &str = &create_article.article.description;
                    let body : &str = &create_article.article.body;
                    let tag_list : Vec<String> = create_article.article.tagList.unwrap_or(Vec::new());
                    let slug : &str = &slugify(title);
                    let tags : &str = &tag_list.join(",");
                    
                    let mut sql = Core::new().unwrap();
                    let create_article_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
                        .and_then(|conn| { conn.query(                            
                            "insert into Tags (Tag) SELECT EmployeeID = Item FROM dbo.SplitNVarchars(@P6, ',')  Except select Tag from Tags;                            
                            INSERT INTO Articles (Title, [Description], Body, Created, Author, Slug) Values (@P1, @P2, @P3, getdate(), @P4, @P5);
                            DECLARE @id int = SCOPE_IDENTITY();
                            insert into [ArticleTags] (ArticleId, TagId) SELECT @id, Id From Tags WHERE Tag IN (SELECT EmployeeID = Item FROM dbo.SplitNVarchars(@P6, ','));
                            SELECT Slug, Title, [Description], Body, Created, Updated, Users.UserName, Users.Bio, Users.[Image], 
                            (SELECT COUNT(*) FROM Followings WHERE FollowerId=@P4 AND Author=FollowingId) as [Following]
                            FROM Articles INNER JOIN Users on Author=Users.Id WHERE Articles.Id  = @id
                            ", 
                            &[&title, &description, &body, &logged_in_user_id, &slug,&tags,]
                            )
                            .for_each_row(|row| {
                                let slug : &str = row.get(0);
                                let title : &str = row.get(1);
                                let description : &str = row.get(2);
                                let body : &str = row.get(3);
                                let created : chrono::NaiveDateTime = row.get(4);
                                let updated : Option<chrono::NaiveDateTime> = row.get(5);
                                let user_name : &str = row.get(6);
                                let bio : Option<&str> = row.get(7);
                                let image : Option<&str> = row.get(8);
                                let f : i32 = row.get(9);
                                let following : bool = f == 1;

                                let profile = Profile{ username: user_name.to_string(), bio:bio.map(|s| s.to_string()),
                                    image:image.map(|s| s.to_string()), following : following };
                                
                                result = Some(Article{ 
                                    slug: slug.to_string(),
                                    title: title.to_string(),
                                    description : description.to_string(),
                                    body : body.to_string(),
                                    tagList: Vec::new(), //TODO: change
                                    createdAt: created,
                                    updatedAt: updated,
                                    favorited : false,
                                    favoritesCount : 0,
                                    author : profile                                    
                                });
                                Ok(())
                            })
                        }
                    );
                    sql.run(create_article_cmd).unwrap(); 
                },
                _ => {
                }
            }
        }
        _ => {

        }
    }
    if result.is_some() {
        let result = result.unwrap();
        let result = serde_json::to_string(&result).unwrap();
        let result : &[u8] = result.as_bytes();
        res.send(&result).unwrap();                        
    }      
}

pub fn favorite_article_handler(mut req: Request, res: Response, c: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);    
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
    let slug = &caps[0].replace("/api/articles/", "").replace("/favorite", "");
    println!("slug: {}", slug);

    let mut result : Option<Article> = None; 
    {
        let mut sql = Core::new().unwrap();
        let get_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
            .and_then(|conn| conn.query(                            
                "declare @id int; select TOP(1) @id = id from Articles where Slug = @P1 ORDER BY 1;
                INSERT INTO [dbo].[FavoritedArticles]
	            ([ArticleId],
	            [UserId])
	            VALUES (@id,@P2);
                SELECT TOP 1 Slug, Title, Description, Body, Created, Updated, UserName, Bio, Image from Articles a 
                INNER JOIN Users u ON a.Author = u.Id
                where a.Id = @id
                "
                , &[&(slug.as_str()), &(logged_id)]
            ).for_each_row(|row| {
                let slug : &str = row.get(0);
                let title : &str = row.get(1);
                let description : &str = row.get(2);
                let body : &str = row.get(3);
                let created_at : NaiveDateTime = row.get(4);
                let updated_at : Option<chrono::NaiveDateTime> = row.get(5);
                let user_name : &str = row.get(6);
                let bio : Option<&str> = row.get(7);
                let image :Option<&str> = row.get(8);
                
                let tag_list : Vec<String> = Vec::new();
                let favorited : bool = true;
                let favorites_count : i32 = 3;
                let author = Profile{ username:user_name.to_string(), bio:bio.map(|s| s.to_string()), image:image.map(|s| s.to_string()), following:false };
                result = Some(Article{ 
                    slug:slug.to_string(), title:title.to_string(), description:description.to_string(), body:body.to_string(), tagList:tag_list, createdAt:created_at, updatedAt:updated_at, favorited:favorited, favoritesCount:favorites_count, author:author
                });
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

pub fn unfavorite_article_handler(mut req: Request, res: Response, c: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);    
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
    let slug = &caps[0].replace("/api/articles/", "").replace("/favorite", "");
    println!("slug: {}", slug);

    let mut result : Option<Article> = None; 
    {
        let mut sql = Core::new().unwrap();
        let get_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
            .and_then(|conn| conn.query(                            
                "declare @id int; 
                select TOP(1) @id = id from Articles where Slug = @P1 ORDER BY 1;
                DELETE TOP(1) FROM FavoritedArticles WHERE ArticleId = @id AND UserId = @P2;
                SELECT TOP 1 Slug, Title, Description, Body, Created, Updated, UserName, Bio, Image from Articles a 
                    INNER JOIN Users u ON a.Author = u.Id
                    where a.Id = @id
                "
                , &[&(slug.as_str()), &(logged_id)]
            ).for_each_row(|row| {
                let slug : &str = row.get(0);
                let title : &str = row.get(1);
                let description : &str = row.get(2);
                let body : &str = row.get(3);
                let created_at : NaiveDateTime = row.get(4);
                let updated_at : Option<chrono::NaiveDateTime> = row.get(5);
                let user_name : &str = row.get(6);
                let bio : Option<&str> = row.get(7);
                let image :Option<&str> = row.get(8);
                
                let tag_list : Vec<String> = Vec::new();
                let favorited : bool = true;
                let favorites_count : i32 = 3;
                let author = Profile{ username:user_name.to_string(), bio:bio.map(|s| s.to_string()), image:image.map(|s| s.to_string()), following:false };
                result = Some(Article{ 
                    slug:slug.to_string(), title:title.to_string(), description:description.to_string(), body:body.to_string(), tagList:tag_list, createdAt:created_at, updatedAt:updated_at, favorited:favorited, favoritesCount:favorites_count, author:author
                });
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

pub fn list_article_handler(mut req: Request, res: Response, c: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body); 

    let caps = c.unwrap();
    let url_params = &caps[0].replace("/api/articles?", "");

    let parsed_params: Vec<&str> = url_params.split('&').collect();

    let mut where_clause = String::new();

    for param in &parsed_params {
        let name_value: Vec<&str> = param.split('=').collect();

        if name_value[0] == "tag" {
            where_clause.push_str("
                INNER JOIN ArticleTags at 
                    ON a.Id = at.ArticleId
                INNER JOIN Users u 
	                ON a.Author = u.Id
                INNER JOIN Tags t
                    ON at.TagId = t.Id AND t.Tag='");
            where_clause.push_str(name_value[1]);
            where_clause.push_str("' ");
        } 
        else if name_value[0] == "author" {
            where_clause.push_str("
                INNER JOIN Users u 
	                ON a.Author = u.Id AND u.UserName = '");
            where_clause.push_str(name_value[1]);
            where_clause.push_str("' ");
        }
        else if name_value[0] == "favorited" {
            where_clause.push_str("
                INNER JOIN FavoritedArticles fa 
	                ON a.Id = fa.ArticleId
                INNER JOIN Users u
	                ON fa.UserId = u.Id AND u.UserName='");
            where_clause.push_str(name_value[1]);
            where_clause.push_str("'");
        };
    }

    let mut select_clause = String::from("SELECT TOP 1 Slug, Title, Description, Body, Created, Updated, UserName, Bio, Image from Articles a ");
    select_clause.push_str(&where_clause);

    println!("select_clause: {}", select_clause);

    let sql_command: Statement = Statement::from(select_clause);

    let mut result : Option<Article> = None; 
    {
        let mut sql = Core::new().unwrap();
        let get_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
            .and_then(|conn| conn.query(sql_command, &[])
            .for_each_row(|row| {
                let slug : &str = row.get(0);
                let title : &str = row.get(1);
                let description : &str = row.get(2);
                let body : &str = row.get(3);
                let created_at : NaiveDateTime = row.get(4);
                let updated_at : Option<chrono::NaiveDateTime> = row.get(5);
                let user_name : &str = row.get(6);
                let bio : Option<&str> = row.get(7);
                let image :Option<&str> = row.get(8);
                
                let tag_list : Vec<String> = Vec::new();
                let favorited : bool = true;
                let favorites_count : i32 = 3;
                let author = Profile{ username:user_name.to_string(), bio:bio.map(|s| s.to_string()), image:image.map(|s| s.to_string()), following:false };
                result = Some(Article{ 
                    slug:slug.to_string(), title:title.to_string(), description:description.to_string(), body:body.to_string(), tagList:tag_list, createdAt:created_at, updatedAt:updated_at, favorited:favorited, favoritesCount:favorites_count, author:author
                });
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


pub fn get_article_handler(mut req: Request, res: Response, c: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);    
    //let token =  req.headers.get::<Authorization<Bearer>>(); 

    let caps = c.unwrap();
    let slug = &caps[0].replace("/api/articles/", "");
    println!("slug: {}", slug);

    let mut result : Option<Article> = None; 
    {
        let mut sql = Core::new().unwrap();
        let get_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
            .and_then(|conn| conn.query(                            
                "SELECT TOP 1 Slug, Title, Description, Body, Created, Updated, UserName, Bio, Image from Articles a 
INNER JOIN Users u ON a.Author = u.Id
where Slug = @p1", &[&(slug.as_str())]
            ).for_each_row(|row| {
                let slug : &str = row.get(0);
                let title : &str = row.get(1);
                let description : &str = row.get(2);
                let body : &str = row.get(3);
                let created_at : NaiveDateTime = row.get(4);
                let updated_at : Option<chrono::NaiveDateTime> = row.get(5);
                let user_name : &str = row.get(6);
                let bio : Option<&str> = row.get(7);
                let image :Option<&str> = row.get(8);
                
                let tag_list : Vec<String> = Vec::new();
                let favorited : bool = true;
                let favorites_count : i32 = 3;
                let author = Profile{ username:user_name.to_string(), bio:bio.map(|s| s.to_string()), image:image.map(|s| s.to_string()), following:false };
                result = Some(Article{ 
                    slug:slug.to_string(), title:title.to_string(), description:description.to_string(), body:body.to_string(), tagList:tag_list, createdAt:created_at, updatedAt:updated_at, favorited:favorited, favoritesCount:favorites_count, author:author
                });
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

pub fn update_article_handler(mut req: Request, res: Response, c: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);    
    let token = req.headers.get::<Authorization<Bearer>>(); 
    let mut result : Option<User> = None; 

    let caps = c.unwrap();
    let slug = &caps[0].replace("/api/articles/", "");
    println!("slug {}", &slug);

    match token {
        Some(token) => {
            let jwt = &token.0.token;
            let logged_in_user_id = login(&jwt);  

            match logged_in_user_id {
                Some(logged_in_user_id) => {
                    println!("logged_in_user {}", &logged_in_user_id);
                    println!("body {}", &body);

                    let update_article : UpdateArticle = serde_json::from_str(&body).unwrap();     
                    let title : Option<String> = update_article.title;
                    let body : Option<String> = update_article.body;
                    let description : Option<String> = update_article.description;
                    
                    if title.is_some() {
                        let t = &title.unwrap();
                        let mut sql = Core::new().unwrap();
                        let s = slugify(t);

                        let update_user_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
                        .and_then(|conn| { conn.query(                            
                            "UPDATE TOP(1) [dbo].[Articles] SET [Title]=@P1, [Slug]=@P2 WHERE [Slug] = @P3; 
                                SELECT TOP 1 Slug, Title, Description, Body, Created, Updated, UserName, Bio, Image from Articles a 
                                INNER JOIN Users u ON a.Author = u.Id
                                where Slug = @P2;", 
                                &[&(t.as_str()), &(s.as_str()), &(slug.as_str())]
                            )
                            .for_each_row(|row| {
                                let email : &str = row.get(0);
                                let token : &str = row.get(1);
                                let user_name : &str = row.get(2);
                                let bio : Option<&str> = row.get(3);
                                let image : Option<&str> = row.get(4);
                                result = Some(User{ 
                                    email:email.to_string(), token:token.to_string(), bio:bio.map(|s| s.to_string()),
                                    image:image.map(|s| s.to_string()), username:user_name.to_string()
                                });
                                Ok(())
                            })
                        }
                    );
                    sql.run(update_user_cmd).unwrap(); 
                    }

                    if body.is_some() {
                        let t = body.unwrap();
                        let mut sql = Core::new().unwrap();

                        let update_user_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
                        .and_then(|conn| { conn.query(                            
                            "UPDATE TOP(1) [dbo].[Articles] SET [Body]=@P1 WHERE [Slug] = @P2; 
                                SELECT TOP 1 Slug, Title, Description, Body, Created, Updated, UserName, Bio, Image from Articles a 
                                INNER JOIN Users u ON a.Author = u.Id
                                where Slug = @P2;", 
                                &[&(t.as_str()), &(slug.as_str())]
                            )
                            .for_each_row(|row| {
                                let email : &str = row.get(0);
                                let token : &str = row.get(1);
                                let user_name : &str = row.get(2);
                                let bio : Option<&str> = row.get(3);
                                let image : Option<&str> = row.get(4);
                                result = Some(User{ 
                                    email:email.to_string(), token:token.to_string(), bio:bio.map(|s| s.to_string()),
                                    image:image.map(|s| s.to_string()), username:user_name.to_string()
                                });
                                Ok(())
                            })
                        }
                    );
                    sql.run(update_user_cmd).unwrap(); 
                    }

                    if description.is_some() {
                        let t = description.unwrap();
                        let mut sql = Core::new().unwrap();

                        let update_user_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
                        .and_then(|conn| { conn.query(                            
                            "UPDATE TOP(1) [dbo].[Articles] SET [Description]=@P1 WHERE [Slug] = @P2; 
                                SELECT TOP 1 Slug, Title, Description, Body, Created, Updated, UserName, Bio, Image from Articles a 
                                INNER JOIN Users u ON a.Author = u.Id
                                where Slug = @P2;", 
                                &[&(t.as_str()), &(slug.as_str())]
                            )
                            .for_each_row(|row| {
                                let email : &str = row.get(0);
                                let token : &str = row.get(1);
                                let user_name : &str = row.get(2);
                                let bio : Option<&str> = row.get(3);
                                let image : Option<&str> = row.get(4);
                                result = Some(User{ 
                                    email:email.to_string(), token:token.to_string(), bio:bio.map(|s| s.to_string()),
                                    image:image.map(|s| s.to_string()), username:user_name.to_string()
                                });
                                Ok(())
                            })
                        }
                    );
                    sql.run(update_user_cmd).unwrap(); 
                    }

                },
                _ => {
                }
            }
        }
        _ => {

        }
    }
    if result.is_some() {
        let result = result.unwrap();
        let result = serde_json::to_string(&result).unwrap();
        let result : &[u8] = result.as_bytes();
        res.send(&result).unwrap();                        
    }      
}

pub fn delete_article_handler(mut req: Request, res: Response, c: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);   

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
    let slug = &caps[0].replace("/api/articles/", "");
    println!("slug: {}", slug);

    let mut sql = Core::new().unwrap();
    let get_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
        .and_then(|conn| conn.query(                            
            "declare @id int; select TOP(1) @id = id from Articles where Slug = @P1 AND Author = @P2 ORDER BY 1; 
            DELETE FROM Comments WHERE ArticleId = @id;
            DELETE FROM FavoritedArticles WHERE ArticleId = @id;
            DELETE FROM ArticleTags WHERE ArticleId = @id;
            DELETE FROM Articles WHERE id = @id AND Author = @P2;
            SELECT 1; 
            ", &[&(slug.as_str()),&(logged_id)]
        ).for_each_row(|row| {
            Ok(())
        })
    );
    sql.run(get_cmd).unwrap(); 
}

#[cfg(test)]
use rand::Rng;

#[cfg(test)]
pub fn login_create_article() -> (std::string::String, std::string::String) {
    let client = Client::new();

    let ( user_name, email ) = register_jacob();
    let jwt = login_jacob( email );    

    let since = since_the_epoch();
    let num = rand::thread_rng().gen_range(0, 1000);    
    let title = format!( "How to train your dragon {}-{}", since, num );   
    let slug : &str = &slugify(title.to_owned());

    let body = format!( r#"{{"article": {{"title": "{}","description": "Ever wonder how?","body": "You have to believe",
                "tagList": ["reactjs", "angularjs", "dragons"]}}}}"#, title);    

    let res = client.post("http://localhost:6767/api/articles")
        .header(Authorization(Bearer {token: jwt.to_owned()}))
        .body(&body)
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
    (jwt, slug.to_string())
}

#[cfg(test)]
#[test]
fn create_article_test() {
    login_create_article();
}

#[cfg(test)]
#[test]
fn favorite_article_test() {
    let client = Client::new();

    let (jwt, title) = login_create_article();
    let url = format!("http://localhost:6767/api/articles/{}/favorite", title);

    let res = client.post(&url)
        .header(Authorization(Bearer {token: jwt}))
        .body("")
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn unfavorite_article_test() {
    let client = Client::new();

    let (jwt, title) = login_create_article();
    let url = format!("http://localhost:6767/api/articles/{}/favorite", title);

    let res = client.delete(&url)
        .header(Authorization(Bearer {token: jwt}))
        .body("")
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn get_article_test() {
    let client = Client::new();

    let (jwt, title) = login_create_article();
    let url = format!("http://localhost:6767/api/articles/{}", title);

    let res = client.get(&url)
        .body("")
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn list_article_test() {
    let client = Client::new();

    let (jwt, title) = login_create_article();

    let res = client.get("http://localhost:6767/api/articles?tag=dragons")
        .body("")
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn update_article_test() {
    let client = Client::new();

    let (jwt, title) = login_create_article();
    let url = format!("http://localhost:6767/api/articles/{}", title);
    let title2 = title + " NOT";
    let body = format!(r#"{{"article": {{"title": "{}","description": "Ever wonder what mistakes you did?","body": "You have to believe he's not going to eat you."}}}}"#, title2);

    let res = client.put(&url)
        .header(Authorization(Bearer {token: jwt}))
        .body(&body)
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn delete_article_test() {
    let client = Client::new();

    let (jwt, title) = login_create_article();
    let url = format!("http://localhost:6767/api/articles/{}", title);

    let res = client.delete(&url)
        .header(Authorization(Bearer {token: jwt}))
        .body("")
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}
