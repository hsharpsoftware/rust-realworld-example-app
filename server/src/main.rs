#[macro_use]
extern crate nickel;

#[macro_use(bson, doc)]
extern crate bson;
extern crate mongodb;

extern crate iis;
extern crate hyper;

extern crate nickel_jwt_session;

extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

extern crate chrono;

extern crate crypto;

extern crate rustc_serialize;

extern crate futures;
extern crate tokio_core;
extern crate tiberius;

extern crate toml;

use futures::Future;
use tokio_core::reactor::Core;
use tiberius::SqlConnection;
use tiberius::stmt::ResultStreamExt;

use nickel::{Nickel, Request, Response, MiddlewareResult, JsonBody};
use nickel::status::StatusCode;

use nickel_jwt_session::{SessionMiddleware, TokenLocation};
use nickel_jwt_session::SessionRequestExtensions;
use nickel_jwt_session::SessionResponseExtensions;

use bson::oid::ObjectId;

use bson::Bson;
use mongodb::{Client, ThreadedClient};
use mongodb::db::ThreadedDatabase;

use chrono::prelude::*;

use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

fn enable_cors<'mw>(_req: &mut Request, mut res: Response<'mw>) -> MiddlewareResult<'mw> {
    // Set appropriate headers

    // see https://github.com/nickel-org/nickel.rs/issues/365#issuecomment-234772648
    res.headers_mut().set_raw("Access-Control-Allow-Origin", vec![b"*".to_vec()]);
    res.headers_mut().set_raw("Access-Control-Allow-Headers", vec![b"Origin X-Requested-With Content-Type Accept".to_vec()]);

    // Pass control to the next middleware
    res.next_middleware()
}

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
#[derive(RustcDecodable, RustcEncodable)]
struct RegistrationDetails {
    email: String,
    username : String,
    password : String
}

#[derive(RustcDecodable, RustcEncodable)]
struct Registration {
    user : RegistrationDetails
}

#[derive(Debug)]
#[derive(RustcDecodable, RustcEncodable)]
struct LoginDetails {
    email: String,
    password : String
}

#[derive(RustcDecodable, RustcEncodable)]
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

fn main() {
    let path = Path::new("conduit.toml");
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

    let database_config = config.database.unwrap();
    let connection_string = database_config.connection_string;
    let databaseName = database_config.database_name;

    let mut server = Nickel::new();
    server.utilize(enable_cors);
    server.utilize(SessionMiddleware::new("conduit").using(TokenLocation::AuthorizationHeader).expiration_time(60 * 30));

    let mut lp = Core::new().unwrap();

    server.utilize(router! {
        get "/" => |_request, response| {
            "<html><body><h1>Hello from <a href='https://github.com/hsharpsoftware/rust-realworld-example-app'>the test application written in Rust on Nickel</a> running in Azure Web App!</h1></body></html>"
        }
        get "/api/test1/:id" => |request, response| {      
            format!("This is test: {:?}", request.param("id"))
        }
        get "/api/test2/:id" => |request, mut response| {      
            // Get the objectId from the request params
            let object_id = request.param("id").unwrap();

            // Match the user id to an bson ObjectId
            let _id = match ObjectId::with_string(object_id) {
                Ok(oid) => {
                    response.set(StatusCode::Ok);
                    return response.send(format!("Test id {} works!", oid))
                }
                Err(e) => {
                    response.set(StatusCode::UnprocessableEntity);
                    let error1 = InternalError { errors : ErrorDetail { message : e.to_string() }  };
                    let j = serde_json::to_string(&error1);
                    return response.send(format!("{}", j.unwrap()))
                }
            };
        }
        get "/api/pwd/:id" => |request, response| {      
            let password = request.param("id");
            format!("hashed password: {:?}", crypto::pbkdf2::pbkdf2_simple(password.unwrap(), 10000).unwrap() )
        }
        post "/api/users" => |request, response| {      
            let registration = request.json_as::<Registration>().unwrap();  

            let mut sql = Core::new().unwrap();
            let insertUser = SqlConnection::connect(sql.handle(), connection_string.unwrap().as_str() )
                .and_then(|conn| conn.simple_query(
                    format!("INSERT INTO [{0}].[dbo].[Users]
                        ([Email]
                        ,[Token]
                        ,[UserName])
                    VALUES
                        ('{1}'
                        ,'{2}'
                        ,'{3}')", databaseName.unwrap(), 
                        str::replace( &registration.user.email, "'", "''" ), 
                        str::replace( &crypto::pbkdf2::pbkdf2_simple(&registration.user.password, 10000).unwrap(), "'", "''" ), 
                        str::replace( &registration.user.username, "'", "''" )
                    )
                ).for_each_row(|row| {Ok(())})
            );
            sql.run(insertUser).unwrap(); 

            format!("Hello {}", 
                registration.user.username 
            )
        }
        post "/api/users/login" => |request, mut response| {      
            let login = request.json_as::<Login>().unwrap();            
            let storedHash = "$rpbkdf2$0$AAAnEA==$Ebk2XzlaoFbX7W7qezg+GA==$NNbdiYlEB5/yZWL+T4oKu40FmQsqBEafi8fPcWuvDV0=$";
            let authenticated_user = crypto::pbkdf2::pbkdf2_check( &login.user.password, &storedHash );
            match authenticated_user {
                Ok(valid) => {
                    if valid {
                        response.set_jwt_user(&login.user.email);
                    } else {
                        response.set(StatusCode::Unauthorized);
                    }
                }
                Err(e) => {
                    response.set(StatusCode::Unauthorized);
                }
            }            
            "".to_string()
        }
        get "/api/user" => |request, mut response| {      
            match request.authorized_user() {
                Some(user) => {
                    // Whatever an authorized user is allowed to do
                    format!("This is test: {:?}", user)
                },
                None => {response.set(StatusCode::Forbidden);"".to_string()}
            }                        
        }
    });

    let createDatabase = SqlConnection::connect(lp.handle(), connection_string.unwrap().as_str() ).and_then(|conn| {
        conn.simple_query(
            format!("IF db_id('{0}') IS NULL CREATE DATABASE [{0}]", databaseName.unwrap())
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
", databaseName.unwrap())
        ).for_each_row(|row| {Ok(())})
    });
    lp.run(createDatabase).unwrap();    

    let port = iis::get_port();

    let listen_on = format!("127.0.0.1:{}", port);

    println!("Listening on {}", listen_on);

    server.listen(listen_on).unwrap();
}
