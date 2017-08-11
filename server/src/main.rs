extern crate bson;

extern crate iis;
extern crate hyper;

extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

extern crate chrono;

extern crate crypto;

extern crate futures;
extern crate tokio_core;
extern crate tiberius;

extern crate toml;

#[macro_use]
extern crate lazy_static;

extern crate reroute;

extern crate jwt;

extern crate futures_state_stream;

extern crate slug;

use futures::Future;
use tokio_core::reactor::Core;
use tiberius::{SqlConnection};
use tiberius::stmt::ResultStreamExt;

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

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
struct User {
    email: String,
    token: String,
    username : String,
    bio : Option<String>,
    image: Option<String>
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
#[allow(non_snake_case)]
struct Article {
    slug: String,
    title: String,
    description : String,
    body : String,
    tagList: Vec<String>,
    createdAt: NaiveDateTime,
    updatedAt: Option<NaiveDateTime>,
    favorited : bool,
    favoritesCount : i32,
    author : Profile
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
struct Profile {
    username: String,
    bio: Option<String>,
    image : Option<String>,
    following : bool
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
#[allow(non_snake_case)]
struct Comment {
    id: i32,
    createdAt: DateTime<UTC>,
    updatedAt: DateTime<UTC>,
    body : String,
    author : Profile
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
struct InternalError {
    errors : ErrorDetail
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
struct ErrorDetail {
    message : String
}

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
struct RegistrationDetails {
    email: String,
    username : String,
    password : String
}

#[derive(Serialize, Deserialize)]
struct Registration {
    user : RegistrationDetails
}

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
struct LoginDetails {
    email: String,
    password : String
}

#[derive(Serialize, Deserialize)]
struct Login {
    user : LoginDetails
}

#[derive(Debug, Deserialize)]
struct Config {
    database: Option<DatabaseConfig>,
}  

#[derive(Debug, Deserialize)]
struct DatabaseConfig {
    connection_string: Option<String>,
    database_name: Option<String>,
    create_database_secret: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
struct UpdateUser {
    user: UpdateUserDetail,
}


#[derive(Serialize, Deserialize)]
#[derive(Debug)]
struct CreateArticle {
    article: CreateArticleDetail
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
#[allow(non_snake_case)]
struct CreateArticleDetail {
    title: String,
    description: String,
    body: String,
    tagList: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
struct UpdateUserDetail {
    email: String,
    username : String,
    bio : Option<String>,
    image: Option<String>
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
struct GetTagsResult {
    tags: Vec<String>,
}

static CONFIG_FILE_NAME : &'static str = r#"conduit.toml"#;

lazy_static! {
    pub static ref CONNECTION_STRING : String = match get_database_config().connection_string {
            Some(cnn) => cnn,
            None => panic!("connection string not present in [database] section in {}", CONFIG_FILE_NAME),
        };
    pub static ref DATABASE_NAME : String = match get_database_config().database_name {
            Some(db_name) => db_name,
            None => panic!("database name not present in [database] section in {}", CONFIG_FILE_NAME),
        };  
    pub static ref CREATE_DATABASE_SECRET : String = match get_database_config().create_database_secret {
            Some(db_name) => db_name,
            None => panic!("create database secret not present in [database] section in {}", CONFIG_FILE_NAME),
        };  
}

fn get_database_config() -> DatabaseConfig {
    let mut path = PathBuf::from(env::current_dir().unwrap());
    path.push(CONFIG_FILE_NAME);
    let display = path.display();

    let mut file = match File::open(&path) {
        Err(why) => panic!("couldn't open {}: {}", display,
                                                   why.description()),
        Ok(file) => file,
    };

    let mut content = String::new();
    match file.read_to_string(&mut content) {
        Err(why) => panic!("couldn't read {}: {}", display,
                                                   why.description()),
        Ok(_) => print!("{} contains:\n{}", display, content),
    }

    let toml_str : &str = &content;
    let config: Config = toml::from_str(toml_str).unwrap();

    let database_config : DatabaseConfig = match config.database {
        Some(database_config) => database_config,
        None => panic!("database not present in {}", CONFIG_FILE_NAME),
    };

    database_config
}

fn new_token(user_id: &str, _: &str) -> Option<String> {
    let header: jwt::Header = Default::default();
    let claims = jwt::Registered {
        iss: Some("mikkyang.com".into()),
        sub: Some(user_id.into()),
        ..Default::default()
    };
    let token = Token::new(header, claims);

    token.signed(b"secret_key", Sha256::new()).ok()
}

fn login(token: &str) -> Option<i32> {
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
 
fn handle_row_no_value(_: tiberius::query::QueryRow) -> tiberius::TdsResult<()> {
    Ok(())
}

fn registration_handler(mut req: Request, _: Response, _: Captures) {
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

#[cfg(test)]
#[test]
fn profile_unlogged_test() {
    let client = Client::new();

    let mut res = client.get("http://localhost:6767/api/profiles/Jacob")
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 
    assert_eq!( buffer, r#"{"username":"Jacob","bio":null,"image":null,"following":false}"# );
    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn follow_test() {
    let client = Client::new();

    let res = client.post("http://localhost:6767/api/users/login")
        .body(r#"{"user":{"email": "jake@jake.jake","password": "jakejake"}}"#)
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
    let token = res.headers.get::<Authorization<Bearer>>().unwrap(); 
    let jwt = &token.0.token;

    let res = client.post("http://localhost:6767/api/profiles/Jacob/follow")
        .header(Authorization(Bearer {token: jwt.to_owned()}))
        .body("")
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}


#[cfg(test)]
#[test]
fn profile_logged_test() {
    let client = Client::new();

    let res = client.post("http://localhost:6767/api/users/login")
        .body(r#"{"user":{"email": "jake@jake.jake","password": "jakejake"}}"#)
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
    let token = res.headers.get::<Authorization<Bearer>>().unwrap(); 
    let jwt = &token.0.token;

    let mut res = client.get("http://localhost:6767/api/profiles/Jacob")
        .header(Authorization(Bearer {token: jwt.to_owned()}))
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 
    //assert_eq!( buffer, r#"{"username":"Jacob","bio":null,"image":null,"following":false}"# );
    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn unfollow_test() {
    let client = Client::new();

    let res = client.post("http://localhost:6767/api/users/login")
        .body(r#"{"user":{"email": "jake@jake.jake","password": "jakejake"}}"#)
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
    let token = res.headers.get::<Authorization<Bearer>>().unwrap(); 
    let jwt = &token.0.token;

    follow_test();

    let res = client.delete("http://localhost:6767/api/profiles/Jacob/follow")
        .header(Authorization(Bearer {token: jwt.to_owned()}))
        .body("")
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn create_article_test() {
    let client = Client::new();

    let res = client.post("http://localhost:6767/api/users/login")
        .body(r#"{"user":{"email": "jake@jake.jake","password": "jakejake"}}"#)
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
    let token = res.headers.get::<Authorization<Bearer>>().unwrap(); 
    let jwt = &token.0.token;

    let res = client.post("http://localhost:6767/api/articles")
        .header(Authorization(Bearer {token: jwt.to_owned()}))
        .body(r#"{"article": {"title": "How to train your dragon","description": "Ever wonder how?","body": "You have to believe",
                "tagList": ["reactjs", "angularjs", "dragons"]}}"#)
        .send()
        .unwrap();
    assert_eq!(res.status, hyper::Ok);
}

#[cfg(test)]
#[test]
fn get_tags_test() {
    let client = Client::new();

    let mut res = client.get("http://localhost:6767/api/tags")
        .send()
        .unwrap();
    let mut buffer = String::new();
    res.read_to_string(&mut buffer).unwrap(); 
    //assert_eq!( buffer, r#"{"username":"Jacob","bio":null,"image":null,"following":false}"# );
    assert_eq!(res.status, hyper::Ok);
}

fn update_user_handler(mut req: Request, res: Response, _: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);    
    let token =  req.headers.get::<Authorization<Bearer>>(); 
    let mut result : Option<User> = None; 
    match token {
        Some(token) => {
            let jwt = &token.0.token;
            let logged_in_user_id = login(&jwt);  

            match logged_in_user_id {
                Some(logged_in_user_id) => {
                    println!("logged_in_user {}", &logged_in_user_id);

                    let update_user : UpdateUser = serde_json::from_str(&body).unwrap();     
                    let user_name : &str = &update_user.user.username;
                    let bio : &str = update_user.user.bio.as_ref().map(|x| &**x).unwrap_or("");
                    let image : &str = update_user.user.image.as_ref().map(|x| &**x).unwrap_or("");
                    let email : &str = &update_user.user.email;

                    let mut sql = Core::new().unwrap();
                    let update_user_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
                        .and_then(|conn| { conn.query(                            
                            "UPDATE [dbo].[Users] SET [UserName]=@P2,[Bio]=@P3,[Image]=@P4, [Email] = @P5 WHERE [Id] = @P1; SELECT [Email],[Token],[UserName],[Bio],[Image] FROM [dbo].[Users] WHERE [Id] = @P1", 
                            &[&logged_in_user_id, &user_name, &bio, &image, &email]
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

fn create_article_handler(mut req: Request, res: Response, _: Captures) {
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

fn test_handler(_: Request, res: Response, _: Captures) {
    res.send(b"Test works.").unwrap();
}

fn hello_handler(_: Request, res: Response, _: Captures) {
    res.send(b"Hello from Rust application in Hyper running in Azure IIS.").unwrap();
}

fn create_db_handler(mut req: Request, mut res: Response, _: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);    
    if body == CREATE_DATABASE_SECRET.as_str() {
        let mut script = String::new();
        let mut f = File::open("database.sql").expect("Unable to open file");
        f.read_to_string(&mut script).expect("Unable to read string");

        let mut lp = Core::new().unwrap();
        let future = SqlConnection::connect(lp.handle(), CONNECTION_STRING.as_str())
        .and_then(|conn| {
            conn.query( script , &[ ]  ).for_each_row( handle_row_no_value )
        } );
        lp.run(future).unwrap();
        res.send(b"Database created.").unwrap();
    } else {
        *res.status_mut() = StatusCode::Unauthorized;        
    }
}

fn get_current_user_handler(req: Request, res: Response, _: Captures) {
    let token = req.headers.get::<Authorization<Bearer>>(); 
    let mut result : Option<User> = None; 
    match token {
        Some(token) => {
            let jwt = &token.0.token;
            let logged_in_user = login(&jwt);  

            match logged_in_user {
                Some(logged_in_user) => {
                    println!("logged_in_user {}", &logged_in_user);
                    let mut sql = Core::new().unwrap();
                    let get_user = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
                        .and_then(|conn| conn.query(                            
                            "SELECT [Email],[Token],[UserName],[Bio],[Image] FROM [dbo].[Users]
                                WHERE [Id] = @P1", &[&logged_in_user]
                        ).for_each_row(|row| {
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
                    );
                    sql.run(get_user).unwrap(); 
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

fn get_profile_handler(req: Request, res: Response, c: Captures) {
    let token = req.headers.get::<Authorization<Bearer>>(); 
    let logged_id : i32 =  
        match token {
            Some(token) => {
                let jwt = &token.0.token;
                login(&jwt).unwrap()

            }
            _ => 0
        };

    let caps = c.unwrap();
    let profile = &caps[0].replace("/api/profiles/", "");
    println!("profile: {}", profile);
    let mut result : Option<Profile> = None; 

    {
        let mut sql = Core::new().unwrap();
        let get_profile_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
            .and_then(|conn| conn.query(                            
                "SELECT TOP 1 [Email],[Token],[UserName],[Bio],[Image] ,
( SELECT COUNT(*) FROM dbo.Followings F WHERE F.[FollowingId] = Id AND F.FollowerId = @P2 ) as Following
FROM [dbo].[Users]  WHERE [UserName] = @P1", &[&(profile.as_str()), &logged_id]
            ).for_each_row(|row| {
                let _ : &str = row.get(0);
                let _ : &str = row.get(1);
                let user_name : &str = row.get(2);
                let bio : Option<&str> = row.get(3);
                let image : Option<&str> = row.get(4);
                let f : i32 = row.get(5);
                let following : bool = f == 1;
                result = Some(Profile{ 
                    following:following, bio:bio.map(|s| s.to_string()),
                    image:image.map(|s| s.to_string()), username:user_name.to_string()
                });
                Ok(())
            })
        );
        sql.run(get_profile_cmd).unwrap(); 
    }

    if result.is_some() {
        let result = result.unwrap();
        let result = serde_json::to_string(&result).unwrap();
        let result : &[u8] = result.as_bytes();
        res.send(&result).unwrap();                        
    }   
}

fn unfollow_handler(req: Request, res: Response, c: Captures) {
    let token = req.headers.get::<Authorization<Bearer>>(); 
    let logged_id : i32 =  
        match token {
            Some(token) => {
                let jwt = &token.0.token;
                login(&jwt).unwrap()

            }
            _ => 0
        };

    let caps = c.unwrap();
    let profile = &caps[0].replace("/api/profiles/", "").replace("/follow", "");
    let mut result : Option<Profile> = None; 

    {
        let mut sql = Core::new().unwrap();
        let delete_user = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
            .and_then(|conn| conn.query(                            
                "DELETE TOP (1) from [dbo].[Followings] WHERE [FollowerId] = @P2;
                SELECT TOP (1) [Email],[Token],[UserName],[Bio],[Image] ,
( SELECT COUNT(*) FROM dbo.Followings F WHERE F.[FollowingId] = Id AND F.FollowerId = @P2 ) as Following
FROM [dbo].[Users]  WHERE [UserName] = @P1", &[&(profile.as_str()), &logged_id]
            )
            .for_each_row(|row| {
                let _ : &str = row.get(0);
                let _ : &str = row.get(1);
                let user_name : &str = row.get(2);
                let bio : Option<&str> = row.get(3);
                let image : Option<&str> = row.get(4);
                let f : i32 = row.get(5);
                let following : bool = f == 1;
                result = Some(Profile{ 
                    following:following, bio:bio.map(|s| s.to_string()),
                    image:image.map(|s| s.to_string()), username:user_name.to_string()
                });
                Ok(())
            })
        );
        sql.run(delete_user).unwrap(); 
    }

    if result.is_some() {
        let result = result.unwrap();
        let result = serde_json::to_string(&result).unwrap();
        let result : &[u8] = result.as_bytes();
        res.send(&result).unwrap();                        
    }   
}

fn follow_handler(req: Request, res: Response, c: Captures) {
    let token = req.headers.get::<Authorization<Bearer>>(); 
    let logged_id : i32 =  
        match token {
            Some(token) => {
                let jwt = &token.0.token;
                login(&jwt).unwrap()

            }
            _ => 0
        };

    let caps = c.unwrap();
    let profile = &caps[0].replace("/api/profiles/", "").replace("/follow", "");
    println!("profile: {}", profile);
    let mut result : Option<Profile> = None; 

    {
        let mut sql = Core::new().unwrap();
        let follow_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
            .and_then(|conn| conn.query(                            
                "INSERT INTO [dbo].[Followings] ([FollowingId] ,[FollowerId])
     SELECT @P2,(SELECT TOP (1) [Id]  FROM [Users] where UserName = @P1) EXCEPT SELECT [FollowingId] ,[FollowerId] from Followings;
                SELECT TOP 1 [Email],[Token],[UserName],[Bio],[Image] ,
( SELECT COUNT(*) FROM dbo.Followings F WHERE F.[FollowingId] = Id AND F.FollowerId = @P2 ) as Following
FROM [dbo].[Users]  WHERE [UserName] = @P1", &[&(profile.as_str()), &logged_id]
            ).for_each_row(|row| {
                let _ : &str = row.get(0);
                let _ : &str = row.get(1);
                let user_name : &str = row.get(2);
                let bio : Option<&str> = row.get(3);
                let image : Option<&str> = row.get(4);
                let f : i32 = row.get(5);
                let following : bool = f == 1;
                result = Some(Profile{ 
                    following:following, bio:bio.map(|s| s.to_string()),
                    image:image.map(|s| s.to_string()), username:user_name.to_string()
                });
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


fn authentication_handler(mut req: Request, mut res: Response, _: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);    
    let login : Login = serde_json::from_str(&body).unwrap();    

    let mut sql = Core::new().unwrap();
    let email : &str = &login.user.email;
    let get_user_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
        .and_then(|conn| conn.query( "SELECT TOP 1 [Token], [Id] FROM [dbo].[Users] WHERE [Email] = @P1", &[&email] )
        .for_each_row(|row| {
            let stored_hash : &str = row.get(0);
            let user_id : i32 = row.get(1);
            let authenticated_user = crypto::pbkdf2::pbkdf2_check( &login.user.password, stored_hash);
            *res.status_mut() = StatusCode::Unauthorized;

            match authenticated_user {
                Ok(valid) => {
                    if valid {                     
                        let token = new_token(user_id.to_string().as_ref(), &login.user.password).unwrap();

                        res.headers_mut().set(
                            Authorization(
                                Bearer {
                                    token: token.to_owned()
                                }
                            )
                        );
                        *res.status_mut() = StatusCode::Ok;
                    }
                }
                _ => {}
            }            
            Ok(())
        })
    );
    sql.run(get_user_cmd).unwrap(); 
}

fn get_tags_handler(_: Request, res: Response, _: Captures) {
    let mut result : Option<GetTagsResult> = None; 

    {
        let mut sql = Core::new().unwrap();
        let get_tags_cmd = SqlConnection::connect(sql.handle(), CONNECTION_STRING.as_str() )
            .and_then(|conn| conn.query(                            
                "SELECT STRING_AGG(Tag, ', ') FROM [dbo].[Tags]", &[]
            ).for_each_row(|row| {
                let all_tags : &str = row.get(0);
                result = Some(GetTagsResult{ 
                    tags: all_tags.split(",").map(|q| q.to_string()).collect()
                });
                Ok(())
            })
        );
        sql.run(get_tags_cmd).unwrap(); 
    }

    if result.is_some() {
        let result = result.unwrap();
        let result = serde_json::to_string(&result).unwrap();
        let result : &[u8] = result.as_bytes();
        res.send(&result).unwrap();                        
    }   
}

fn main() {    
    let port = iis::get_port();

    let listen_on = format!("0.0.0.0:{}", port);

    println!("Listening on {}", listen_on);

    let mut builder = RouterBuilder::new();

    // Use raw strings so you don't need to escape patterns.
    builder.get(r"/", hello_handler);   
    builder.post(r"/createdb", create_db_handler);   
    builder.post(r"/api/users/login", authentication_handler);   
    builder.post(r"/api/users", registration_handler);   
    builder.get(r"/api/user", get_current_user_handler);   
    builder.get(r"/test", test_handler);   
    builder.put(r"/api/user", update_user_handler);   
    builder.get(r"/api/profiles/.*", get_profile_handler);   
    builder.post(r"/api/profiles/.*", follow_handler);   
    builder.delete(r"/api/profiles/.*", unfollow_handler);  
    builder.post(r"/api/articles", create_article_handler);   
    builder.get(r"/api/profiles/.*", get_profile_handler);   
    builder.get(r"/api/tags", get_tags_handler);   

    let router = builder.finalize().unwrap(); 

    Server::http(listen_on).unwrap().handle(router).unwrap();  

}
