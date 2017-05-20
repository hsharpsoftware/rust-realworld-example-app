#[macro_use(bson, doc)]
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

use futures::Future;
use tokio_core::reactor::Core;
use tiberius::SqlConnection;
use tiberius::stmt::ResultStreamExt;

use bson::oid::ObjectId;

use bson::Bson;

use chrono::prelude::*;

use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::env;
use std::path::PathBuf;

use hyper::server::{Server, Request, Response};

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
struct User {
    email: String,
    token: String,
    username : String,
    bio : String,
    image: String
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
    createdAt: DateTime<UTC>,
    updatedAt: DateTime<UTC>,
    favorited : bool,
    favoritesCount : i32,
    author : Profile
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
struct Profile {
    username: String,
    bio: String,
    image : String,
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
}

static config_file_name : &'static str = r#"conduit.toml"#;

lazy_static! {
    pub static ref connection_string : String = match get_database_config().connection_string {
            Some(cnn) => cnn,
            None => panic!("connection string not present in [database] section in {}", config_file_name),
        };
    pub static ref databaseName : String = match get_database_config().database_name {
            Some(dbName) => dbName,
            None => panic!("database name not present in [database] section in {}", config_file_name),
        };  
}

fn get_database_config() -> DatabaseConfig {
    let mut path = PathBuf::from(env::current_dir().unwrap());
    path.push(config_file_name);
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

    let mut database_config : DatabaseConfig = match config.database {
        Some(database_config) => database_config,
        None => panic!("database not present in {}", config_file_name),
    };

    database_config
}

fn hello(req: Request, res: Response) {
    // handle things here
}

fn main() {    
    let mut lp = Core::new().unwrap();
    let createDatabase = SqlConnection::connect(lp.handle(), connection_string.as_str() ).and_then(|conn| {
            conn.simple_query(
                format!("IF db_id('{0}') IS NULL CREATE DATABASE [{0}]", &**databaseName)
            ).for_each_row(|row| {Ok(())})
        }).and_then( |conn| {
            conn.simple_query(
                format!("if object_id('{0}..Users') is null CREATE TABLE [{0}].[dbo].[Users](
        [Id] [int] IDENTITY(1,1) NOT NULL,
        [Email] [nvarchar](50) NOT NULL,
        [Token] [varchar](250) NOT NULL,
        [UserName] [nvarchar](150) NOT NULL,
        [Bio] [nvarchar](max) NULL,
        [Image] [nchar](250) NULL,
    CONSTRAINT [PK_Users] PRIMARY KEY CLUSTERED 
    (
        [Id] ASC
    )WITH (PAD_INDEX = OFF, STATISTICS_NORECOMPUTE = OFF, IGNORE_DUP_KEY = OFF, ALLOW_ROW_LOCKS = ON, ALLOW_PAGE_LOCKS = ON) ON [PRIMARY]
    ) ON [PRIMARY] TEXTIMAGE_ON [PRIMARY]
    ", &**databaseName )
            ).for_each_row(|row| {Ok(())})
        });
        lp.run(createDatabase).unwrap(); 

    let port = iis::get_port();

    let listen_on = format!("0.0.0.0:{}", port);

    println!("Listening on {}", listen_on);

    Server::http(listen_on).unwrap().handle(hello).unwrap();    

}
