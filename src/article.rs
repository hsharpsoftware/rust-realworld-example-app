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

use std::io::prelude::*;

use hyper::server::{Request, Response};
use reroute::{Captures};

use slug::slugify;

use super::*;

static ARTICLE_SELECT : &'static str = r#"
  SELECT Slug, Title, [Description], Body, Created, Updated, Users.UserName, Users.Bio, Users.[Image], 
                (SELECT COUNT(*) FROM Followings WHERE FollowerId=@logged AND Author=FollowingId) as [Following],
                (SELECT COUNT(*) FROM FavoritedArticles WHERE ArticleId = @id ) as FavoritesCount,
                (SELECT COUNT(*) FROM FavoritedArticles WHERE UserId = @logged ) as PersonalFavoritesCount,
				(SELECT STRING_AGG(Tag, ',') FROM [Tags] inner join ArticleTags on ArticleTags.TagId = Tags.Id where ArticleId=@id)  as Tags
                FROM Articles INNER JOIN Users on Author=Users.Id  WHERE Articles.Id = @id
"#;

fn get_simple_article_from_row( row : tiberius::query::QueryRow ) -> Option<Article> {
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
    let favorites_count: i32 = row.get(10);
    let personal_favorite_count: i32 = row.get(11);
    let favorited : bool = personal_favorite_count > 0;
    let tags_combined : &str = row.get(12);

    let profile = Profile{ username: user_name.to_string(), bio:bio.map(|s| s.to_string()),
        image:image.map(|s| s.to_string()), following : following };
    
    let result = Article{ 
        slug: slug.to_string(),
        title: title.to_string(),
        description : description.to_string(),
        body : body.to_string(),
        tagList: tags_combined.split(",").map(|q| q.to_string()).collect(),
        createdAt: created,
        updatedAt: updated,
        favorited : favorited,
        favoritesCount : favorites_count,
        author : profile                                    
    };
    Some(result)
}
fn get_article_from_row( row : tiberius::query::QueryRow ) -> Option<CreateArticleResult> {
    Some(CreateArticleResult{ article:get_simple_article_from_row(row).unwrap() })
}
pub fn create_article_handler(req: Request, res: Response, _: Captures) {
    let (body, logged_in_user_id) = prepare_parameters(req);
    
    let create_article : CreateArticle = serde_json::from_str(&body).unwrap();     
    let title : &str = &create_article.article.title;
    let description : &str = &create_article.article.description;
    let body : &str = &create_article.article.body;
    let tag_list : Vec<String> = create_article.article.tagList.unwrap_or(Vec::new());
    let slug : &str = &slugify(title);
    let tags : &str = &tag_list.join(",");

    process(
        res,
        r#"insert into Tags (Tag) SELECT EmployeeID = Item FROM dbo.SplitNVarchars(@P6, ',')  Except select Tag from Tags;                            
        INSERT INTO Articles (Title, [Description], Body, Created, Author, Slug) Values (@P1, @P2, @P3, getdate(), @P4, @P5);
        DECLARE @id int = SCOPE_IDENTITY(); DECLARE @logged int = @P4;
        insert into [ArticleTags] (ArticleId, TagId) SELECT @id, Id From Tags WHERE Tag IN (SELECT EmployeeID = Item FROM dbo.SplitNVarchars(@P6, ','));
        "#, 
        ARTICLE_SELECT,
        get_article_from_row,
        &[&title, &description, &body, &logged_in_user_id, &slug,&tags,]
    );
}

fn process_and_return_article(name : &str, req: Request, res: Response, c: Captures, sql_command : &'static str ) {
    let (_, logged_id) = prepare_parameters( req );
    
    let caps = c.unwrap();
    let slug = &caps[0].replace("/api/articles/", "").replace("/favorite", "");
    println!("{} slug: '{}'", name, slug);
    println!("logged_id: {}", logged_id);

    process(
        res,
        sql_command,
        ARTICLE_SELECT,
        get_article_from_row,
        &[&(slug.as_str()), &(logged_id)]
    ); 
}

pub fn favorite_article_handler(req: Request, res: Response, c: Captures) {
    process_and_return_article("favorite_article_handler", req, res, c, "declare @id int; select TOP(1) @id = id from Articles where Slug = @P1 ORDER BY 1; DECLARE @logged int = @P2;
                INSERT INTO [dbo].[FavoritedArticles]
	            ([ArticleId],
	            [UserId])
	            VALUES (@id,@P2)");              
}

pub fn unfavorite_article_handler(req: Request, res: Response, c: Captures) {
    process_and_return_article("unfavorite_article_handler", req, res, c, "declare @id int; DECLARE @logged int = @P2;
                select TOP(1) @id = id from Articles where Slug = @P1 ORDER BY 1;
                DELETE TOP(1) FROM FavoritedArticles WHERE ArticleId = @id AND UserId = @P2;
                ");
}

fn articles_result( _ : ArticlesResult ) {}

pub fn feed_handler(req: Request, res: Response, c: Captures) {
    let (_, logged_id) = prepare_parameters( req );

    let caps = c.unwrap();
    let url_params = &caps[0].replace("/api/articles/feed?", "");

    println!("feed_handler url_params:'{}'", url_params);

    let parsed_params: Vec<&str> = url_params.split('&').collect();

    let mut limit :i32 = 20;
    let mut offset :i32 = 0;

    for param in &parsed_params {
        let name_value: Vec<&str> = param.split('=').collect();

        if name_value[0] == "offset" {
            offset = name_value[1].parse::<i32>().unwrap();
        }
        else if name_value[0] == "limit" {
            limit = name_value[1].parse::<i32>().unwrap();
        }
        ;
    }    

    process_container(
        res,
        r#"declare @logged int = @p1;
        "#,
        r#"SELECT Slug, Title, [Description], Body, Created, Updated, Users.UserName, Users.Bio, Users.[Image], 
                (SELECT COUNT(*) FROM Followings WHERE FollowerId=@logged AND Author=FollowingId) as [Following],
                (SELECT COUNT(*) FROM FavoritedArticles WHERE ArticleId = Articles.Id ) as FavoritesCount,
                (SELECT COUNT(*) FROM FavoritedArticles WHERE UserId = @logged ) as PersonalFavoritesCount,
				(SELECT STRING_AGG(Tag, ',') FROM [Tags] inner join ArticleTags on ArticleTags.TagId = Tags.Id where ArticleId=Articles.Id)  as Tags
                FROM Articles INNER JOIN Users on Author=Users.Id  
				WHERE Author IN ( SELECT FollowingId FROM Followings WHERE FollowerId = @logged ) 
order by Articles.Id DESC OFFSET @p2 ROWS FETCH NEXT @p3 ROWS Only"#,
        get_simple_article_from_row,
        articles_result,
        &[&logged_id, &offset, &limit]
    );
}

pub fn list_article_handler(mut req: Request, res: Response, c: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body); 

    let caps = c.unwrap();
    let url_params = &caps[0].replace("/api/articles?", "");

    println!("list_article_handler url_params:'{}'", url_params);

    let parsed_params: Vec<&str> = url_params.split('&').collect();

    let mut where_clause = String::new();

    let mut limit = "20";
    let mut offset = "0";

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
        }
        else if name_value[0] == "offset" {
            offset = name_value[1];
        }
        else if name_value[0] == "limit" {
            limit = name_value[1];
        }
        ;
    }

    let mut select_clause = String::from("SELECT Slug, Title, Description, Body, Created, Updated, UserName, Bio, Image from Articles a ");
    select_clause.push_str(&where_clause);
    select_clause.push_str("order by a.Id ASC OFFSET ");
    select_clause.push_str(&offset);
    select_clause.push_str("  ROWS ");
    select_clause.push_str("FETCH NEXT ");
    select_clause.push_str(&limit);
    select_clause.push_str(" ROWS ONLY");

    println!("select_clause: {}", select_clause);

    let sql_command: Statement = Statement::from(select_clause);

    let mut articles : Vec<Article> = Vec::new(); 
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
                let art = Article{ 
                    slug:slug.to_string(), title:title.to_string(), description:description.to_string(), body:body.to_string(), tagList:tag_list, createdAt:created_at, updatedAt:updated_at, favorited:favorited, favoritesCount:favorites_count, author:author
                };
                articles.push(art);
                Ok(())
            })
        );
        sql.run(get_cmd).unwrap(); 
    }

    let result = ArticlesResult{articles:articles};
    let result = serde_json::to_string(&result).unwrap();
    let result : &[u8] = result.as_bytes();
    res.send(&result).unwrap();                        
}

pub fn get_article_handler(req: Request, res: Response, c: Captures) {
    process_and_return_article(
        "get_article_handler", req, res, c, 
        "declare @id int; select TOP(1) @id = id from Articles where Slug = @P1 ORDER BY 1; 
        DECLARE @logged int = @P2;");              
}

pub fn update_article_handler(req: Request, res: Response, c: Captures) {
    let (body, logged_id) = prepare_parameters(req);

    let caps = c.unwrap();
    let slug = &caps[0].replace("/api/articles/", "");
    println!("slug {}", &slug);

    let update_article : UpdateArticle = serde_json::from_str(&body).unwrap();     
    let title : &str = update_article.article.title.as_ref().map(|x| &**x).unwrap_or("");
    let body : &str = update_article.article.body.as_ref().map(|x| &**x).unwrap_or("");
    let description : &str = update_article.article.description.as_ref().map(|x| &**x).unwrap_or("");
    let new_slug : &str = &slugify(title);

    process(
        res,
        r#"
        declare @id int; select TOP(1) @id = id from Articles where Slug = @P1; 
        DECLARE @logged int = @P5;
        UPDATE TOP(1) [dbo].[Articles] SET 
        [Title]=CASE WHEN(LEN(@P2)=0) THEN Title ELSE @P2 END,
        [Description]=CASE WHEN(LEN(@P3)=0) THEN Description ELSE @P3 END,
        [Body]=CASE WHEN(LEN(@P4)=0) THEN Description ELSE @P4 END,
        [Slug]=CASE WHEN(LEN(@P2)=0) THEN [Slug] ELSE @P6 END
        WHERE [Id] = @id AND Author = @logged; 
        "#, 
        ARTICLE_SELECT,
        get_article_from_row,
        &[&(slug.as_str()), &title, &description, &body, &logged_id, &new_slug]
    );
}

pub fn delete_article_handler(req: Request, res: Response, c: Captures) {
    let (_, logged_id) = prepare_parameters( req );

    let caps = c.unwrap();
    let slug = &caps[0].replace("/api/articles/", "");
    println!("slug: {}", slug);

    process(
        res,
        "declare @id int; select TOP(1) @id = id from Articles where Slug = @P1 AND Author = @P2 ORDER BY 1; 
        DELETE FROM Comments WHERE ArticleId = @id;
        DELETE FROM FavoritedArticles WHERE ArticleId = @id;
        DELETE FROM ArticleTags WHERE ArticleId = @id;
        DELETE FROM Articles WHERE id = @id AND Author = @P2;",
        "SELECT 1",
        handle_row_none,
        &[&(slug.as_str()),&(logged_id)]
    );
}

#[cfg(test)]
use rand::Rng;

#[cfg(test)]
pub fn login_create_article(follow:bool) -> (std::string::String, std::string::String, std::string::String) {
    let client = Client::new();

    let ( user_name, _, jwt ) = 
        if follow { 
            user::follow_jacob() 
        } else { 
            let ( user_name, email ) = register_jacob() ;
            let jwt = login_jacob( email.to_owned(), user::JACOB_PASSWORD.to_string() );  
            ( user_name, email, jwt )
        };

    let since = since_the_epoch();
    let num = rand::thread_rng().gen_range(0, 1000);    
    let title = format!( "How to train your dragon {}-{}", since, num );   
    let slug : &str = &slugify(title.to_owned());

    let body = format!( r#"{{"article": {{"title": "{}","description": "Ever wonder how?","body": "You have to believe",
                "tagList": ["reactjs", "angularjs", "dragons"]}}}}"#, title);    

    let mut res = client.post("http://localhost:6767/api/articles")
        .header(Authorization(Bearer {token: jwt.to_owned()}))
        .body(&body)
        .send()
        .unwrap();

    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 

    let create_result : CreateArticleResult = serde_json::from_str(&buffer).unwrap();   
    let article = create_result.article;  
    assert_eq!(article.title, title); 
    assert_eq!(article.slug, slug);
    assert_eq!(article.favorited, false);
    assert_eq!(article.author.username, user_name);
    assert_eq!(article.tagList.len(), 3);

    assert_eq!(res.status, hyper::Ok);

    (jwt, slug.to_string(), user_name)
}

#[cfg(test)]
#[test]
fn create_article_test() {
    login_create_article(false);
}

#[cfg(test)]
#[test]
fn favorite_article_test() {
    let client = Client::new();

    let (jwt, slug, user_name) = login_create_article(false);
    let url = format!("http://localhost:6767/api/articles/{}/favorite", slug);

    let mut res = client.post(&url)
        .header(Authorization(Bearer {token: jwt}))
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 
        
    let create_result : CreateArticleResult = serde_json::from_str(&buffer).unwrap();   
    let article = create_result.article;  
    assert_eq!(article.slug, slug);
    assert_eq!(article.favorited, true);
    assert_eq!(article.favoritesCount, 1);
    assert_eq!(article.author.username, user_name);

    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn unfavorite_article_test() {
    let client = Client::new();

    let (jwt, slug, user_name) = login_create_article(false);
    let url = format!("http://localhost:6767/api/articles/{}/favorite", slug);

    let mut res = client.delete(&url)
        .header(Authorization(Bearer {token: jwt}))
        .body("")
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 
        
    let create_result : CreateArticleResult = serde_json::from_str(&buffer).unwrap();   
    let article = create_result.article;  
    assert_eq!(article.slug, slug);
    assert_eq!(article.favorited, false);
    assert_eq!(article.favoritesCount, 0);
    assert_eq!(article.author.username, user_name);

    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn get_article_test() {
    let client = Client::new();

    let (_, slug, user_name) = login_create_article(false);
    let url = format!("http://localhost:6767/api/articles/{}", slug);

    let mut res = client.get(&url)
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 
        

    let create_result : CreateArticleResult = serde_json::from_str(&buffer).unwrap();   
    let article = create_result.article;  
    assert_eq!(article.slug, slug);    
    assert_eq!(article.favorited, false);
    assert_eq!(article.favoritesCount, 0);
    assert_eq!(article.author.username, user_name);

    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn list_article_test() {
    let client = Client::new();

    let (_, _, _) = login_create_article(false);

    let mut res = client.get("http://localhost:6767/api/articles?tag=dragons")
        .body("")
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);

    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 

    let articles : ArticlesResult = serde_json::from_str(&buffer).unwrap();       
    assert_eq!(articles.articles.len()>0, true);
}

#[cfg(test)]
#[test]
fn unfollowed_feed_article_test() {
    let client = Client::new();

    let (jwt, _, _) = login_create_article(false);

    let mut res = client.get("http://localhost:6767/api/articles/feed")
        .header(Authorization(Bearer {token: jwt}))
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);

    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 

    let articles : ArticlesResult = serde_json::from_str(&buffer).unwrap();       
    assert_eq!(articles.articles.len()==0, true);
}

#[cfg(test)]
#[test]
fn followed_feed_article_test() {
    let client = Client::new();

    let (jwt, _, _) = login_create_article(true);

    let mut res = client.get("http://localhost:6767/api/articles/feed")
        .header(Authorization(Bearer {token: jwt}))
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);

    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 

    let articles : ArticlesResult = serde_json::from_str(&buffer).unwrap();       
    assert_eq!(articles.articles.len()==1, true);
}

#[cfg(test)]
#[test]
fn update_article_test() {
    let client = Client::new();

    let (jwt, title, user_name) = login_create_article(false);
    let url = format!("http://localhost:6767/api/articles/{}", title);
    let title2 = title + " NOT";
    let body = format!(r#"{{"article": {{"title": "{}","description": "CHANGED1","body": "CHANGED2"}}}}"#, title2);

    let mut res = client.put(&url)
        .header(Authorization(Bearer {token: jwt}))
        .body(&body)
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 

    let create_result : CreateArticleResult = serde_json::from_str(&buffer).unwrap();   
    let article = create_result.article;
    assert_eq!(article.slug, slugify(title2.to_owned()));
    assert_eq!(article.title, title2);
    assert_eq!(article.description, "CHANGED1");
    assert_eq!(article.body, "CHANGED2");
    assert_eq!(article.favorited, false);
    assert_eq!(article.favoritesCount, 0);
    assert_eq!(article.author.username, user_name);
}

#[cfg(test)]
#[test]
fn delete_article_test() {
    let client = Client::new();

    let (jwt, title, _) = login_create_article(false);
    let url = format!("http://localhost:6767/api/articles/{}", title);

    let res = client.delete(&url)
        .header(Authorization(Bearer {token: jwt}))
        .body("")
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}
