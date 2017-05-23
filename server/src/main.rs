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

extern crate reroute;

extern crate jwt;

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
use reroute::{RouterBuilder, Captures};
use hyper::header::{Authorization, Bearer};
use hyper::{Get, Post};
use hyper::status::StatusCode;

use crypto::sha2::Sha256;

use jwt::{
    Header,
    Registered,
    Token,
};

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

fn new_token(user_id: &str, password: &str) -> Option<String> {
    let header: jwt::Header = Default::default();
    let claims = jwt::Registered {
        iss: Some("mikkyang.com".into()),
        sub: Some(user_id.into()),
        ..Default::default()
    };
    let token = Token::new(header, claims);

    token.signed(b"secret_key", Sha256::new()).ok()
}

fn login(token: &str) -> Option<String> {
    let token = Token::<Header, Registered>::parse(token).unwrap();

    if token.verify(b"secret_key", Sha256::new()) {
        token.claims.sub
    } else {
        None
    }
}

fn registration_handler(mut req: Request, mut res: Response, _: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);    
    let registration : Registration = serde_json::from_str(&body).unwrap();     

    let mut sql = Core::new().unwrap();
    let insertUser = SqlConnection::connect(sql.handle(), connection_string.as_str() )
        .and_then(|conn| conn.simple_query(
            format!("INSERT INTO [dbo].[Users]
                ([Email]
                ,[Token]
                ,[UserName])
            VALUES
                ('{1}'
                ,'{2}'
                ,'{3}') --{0}", &**databaseName, 
                str::replace( &registration.user.email, "'", "''" ), 
                str::replace( &crypto::pbkdf2::pbkdf2_simple(&registration.user.password, 10000).unwrap(), "'", "''" ), 
                str::replace( &registration.user.username, "'", "''" )
            )
        ).for_each_row(|row| {Ok(())})
    );
    sql.run(insertUser).unwrap();     
}

fn get_current_user_handler(mut req: Request, res: Response, _: Captures) {
    let token = req.headers.get::<Authorization<Bearer>>(); 
    match token {
        Some(token) => {
            let jwt = &token.0.token;
            let logged_in_user = login(&jwt);  
            let mut result : Option<User> = None; 

            match logged_in_user {
                Some(logged_in_user) => {
                    println!("logged_in_user {}", &logged_in_user);
                    let mut sql = Core::new().unwrap();
                    let getUser = SqlConnection::connect(sql.handle(), connection_string.as_str() )
                        .and_then(|conn| conn.simple_query(                            
                            format!("SELECT [Email],[Token],[UserName],[Bio],[Image] FROM [dbo].[Users]
                                WHERE [Email] = '{1}' --{0}", &**databaseName, 
                                str::replace( &logged_in_user, "'", "''" )
                            )
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
                    sql.run(getUser).unwrap(); 
                    res.send(b"Hello World!").unwrap();
                },
                _ => {
                }
            }
        }
        _ => {

        }
    }
}

fn test_handler(mut req: Request, mut res: Response, _: Captures) {
    res.send(b"Test works.").unwrap();
}

fn authentication_handler(mut req: Request, mut res: Response, _: Captures) {
    let mut body = String::new();
    let _ = req.read_to_string(&mut body);    
    let login : Login = serde_json::from_str(&body).unwrap();    

    let mut sql = Core::new().unwrap();
    let getUser = SqlConnection::connect(sql.handle(), connection_string.as_str() )
        .and_then(|conn| conn.simple_query(
            format!("SELECT [Token] FROM [dbo].[Users]
                WHERE [Email] = '{1}' --{0}", &**databaseName, 
                str::replace( &login.user.email, "'", "''" )
            )
        ).for_each_row(|row| {
            let storedHash : &str = row.get(0);
            let authenticated_user = crypto::pbkdf2::pbkdf2_check( &login.user.password, storedHash );
            *res.status_mut() = StatusCode::Unauthorized;

            match authenticated_user {
                Ok(valid) => {
                    if valid {                     
                        let token = new_token(&login.user.email, &login.user.password).unwrap();

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
    sql.run(getUser).unwrap(); 
}

fn main() {    
    let mut lp = Core::new().unwrap();
    /*
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
        [Image] [nvarchar](250) NULL,
    CONSTRAINT [PK_Users] PRIMARY KEY CLUSTERED 
    (
        [Id] ASC
    )WITH (PAD_INDEX = OFF, STATISTICS_NORECOMPUTE = OFF, IGNORE_DUP_KEY = OFF, ALLOW_ROW_LOCKS = ON, ALLOW_PAGE_LOCKS = ON) ON [PRIMARY]
    ) ON [PRIMARY] TEXTIMAGE_ON [PRIMARY]
    ", &**databaseName )
            ).for_each_row(|row| {Ok(())})
        });
        lp.run(createDatabase).unwrap(); 
    */
    let port = iis::get_port();

    let listen_on = format!("0.0.0.0:{}", port);

    println!("Listening on {}", listen_on);

    let mut builder = RouterBuilder::new();

    // Use raw strings so you don't need to escape patterns.
    builder.post(r"/api/users/login", authentication_handler);   
    builder.post(r"/api/users", registration_handler);   
    builder.get(r"/api/user", get_current_user_handler);   
    builder.get(r"/test", test_handler);   

    let router = builder.finalize().unwrap(); 

    Server::http(listen_on).unwrap().handle(router).unwrap();  

}
