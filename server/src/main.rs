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

use std::time::{SystemTime, UNIX_EPOCH};

pub fn since_the_epoch() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
    since_the_epoch.as_secs() * 1000 + since_the_epoch.subsec_nanos() as u64 / 1_000_000
}

#[cfg(test)]
use hyper::Client;

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
struct UpdateArticle {
    title: Option<String>,
    description : Option<String>,
    body : Option<String>
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
    createdAt: NaiveDateTime,
    updatedAt: NaiveDateTime,
    body : String,
    author : Profile
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
#[allow(non_snake_case)]
struct CommentResult {
    comment: Comment,
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
#[allow(non_snake_case)]
struct CommentsResult {
    comments: Vec<Comment>,
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
pub struct Registration {
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
struct AddComment {
    comment: AddCommentDetail,
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
#[allow(non_snake_case)]
struct AddCommentDetail {
    body: String,
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

    let env_config = 
        match env::var("DATABASECONFIG") {
            Ok(lang) => lang,
            Err(_) => "".to_string(),
        };
    let mut content = env_config.replace( "&&&", "\n");

    let mut path = PathBuf::from(env::current_dir().unwrap());
    path.push(CONFIG_FILE_NAME);
    let display = path.display();

    if path.exists() {
        let mut file = match File::open(&path) {
            Err(why) => panic!("couldn't open {}: {}", display,
                                                    why.description()),
            Ok(file) => file,
        };

        match file.read_to_string(&mut content) {
            Err(why) => panic!("couldn't read {}: {}", display,
                                                    why.description()),
            Ok(_) => print!("{} contains:\n{}", display, content),
        }
    }

    let toml_str : &str = &content;
    let config: Config = toml::from_str(toml_str).unwrap();

    let database_config : DatabaseConfig = match config.database {
        Some(database_config) => database_config,
        None => panic!("database not present in {}", CONFIG_FILE_NAME),
    };

    database_config
}

mod user;
use user::*;
 
mod article;
use article::*;

mod comment;
use comment::*;

fn handle_row_no_value(_: tiberius::query::QueryRow) -> tiberius::TdsResult<()> {
    Ok(())
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

    let listen_on = format!("127.0.0.1:{}", port);

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
    builder.post(r"/api/profiles/.*/follow", follow_handler);   
    builder.delete(r"/api/profiles/.*/follow", unfollow_handler);  
    builder.post(r"/api/articles", create_article_handler);   
    builder.get(r"/api/tags", get_tags_handler);   
    builder.post(r"/api/articles/.*/comments", add_comment_handler);  
    builder.post(r"/api/articles/.*/favorite", favorite_article_handler);  
    builder.delete(r"/api/articles/.*/favorite", unfavorite_article_handler);
    builder.put(r"/api/articles/.*", update_article_handler);   
    builder.get(r"/api/articles/.*", get_article_handler);  
    builder.get(r"/api/articles?.*", list_article_handler); 
    builder.delete(r"/api/articles/.*/comments/.*", delete_comment_handler);  
    builder.delete(r"/api/articles/.*", delete_article_handler);  
    builder.get(r"/api/articles/.*/comments/.*", get_comments_handler);  

    let router = builder.finalize().unwrap(); 

    Server::http(listen_on).unwrap().handle(router).unwrap();  

}
