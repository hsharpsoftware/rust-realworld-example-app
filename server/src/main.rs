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

fn main() {
    let mut server = Nickel::new();
    server.utilize(enable_cors);
    server.utilize(SessionMiddleware::new("conduit").using(TokenLocation::AuthorizationHeader).expiration_time(60 * 30));

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

    let mut lp = Core::new().unwrap();
    let connection_string = r#"server=tcp:127.0.0.1,1433;integratedSecurity=true;"#.to_owned();

    let databaseName = "Conduit";

    let future = SqlConnection::connect(lp.handle(), connection_string.as_str()).and_then(|conn| {
        conn.simple_query("SELECT 1+2").for_each_row(|row| {
            let val: i32 = row.get(0);
            assert_eq!(val, 3i32);
            Ok(())
        })
    });
    lp.run(future).unwrap();
    let future2 = SqlConnection::connect(lp.handle(), connection_string.as_str()).and_then(|conn| {
        conn.simple_query("SELECT 3+4").for_each_row(|row| {
            let val: i32 = row.get(0);
            assert_eq!(val, 7i32);
            Ok(())
        })
    });
    lp.run(future2).unwrap();

    let dropDatabase = SqlConnection::connect(lp.handle(), connection_string.as_str()).and_then(|conn| {
        conn.simple_query(
            format!("WHILE EXISTS(select NULL from sys.databases where name='{0}')
BEGIN
    DECLARE @SQL varchar(max)
    SELECT @SQL = COALESCE(@SQL,'') + 'Kill ' + Convert(varchar, SPId) + ';'
    FROM MASTER..SysProcesses
    WHERE DBId = DB_ID(N'{0}') AND SPId <> @@SPId
    EXEC(@SQL)
    DROP DATABASE [{0}]
END", databaseName)
        ).for_each_row(|row| {
            Ok(())
        })
    });
    lp.run(dropDatabase).unwrap();
    let createDatabase = SqlConnection::connect(lp.handle(), connection_string.as_str()).and_then(|conn| {
        conn.simple_query(
            format!("CREATE DATABASE {0}", databaseName)
        ).for_each_row(|row| {
            Ok(())
        })
    });
    lp.run(createDatabase).unwrap();    

    let port = iis::get_port();

    let listen_on = format!("127.0.0.1:{}", port);

    println!("Listening on {}", listen_on);

    server.listen(listen_on).unwrap();
}
