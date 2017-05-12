#[macro_use]
extern crate nickel;

#[macro_use(bson, doc)]
extern crate bson;
extern crate mongodb;

extern crate iis;
extern crate hyper;

extern crate nickel_jwt_session;

extern crate serde;
#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

extern crate chrono;

use nickel::{Nickel, HttpRouter, Request, Response, MiddlewareResult, NickelError};
use hyper::header::{AccessControlAllowOrigin, AccessControlAllowHeaders, Headers};
use nickel::status::StatusCode;

use nickel_jwt_session::{SessionMiddleware, TokenLocation};

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
struct Comment {
    id: i32,
    createdAt: DateTime<UTC>,
    updatedAt: DateTime<UTC>,
    body : String,
    author : Profile
}

fn main() {
    let mut server = Nickel::new();
    server.utilize(enable_cors);
    server.utilize(SessionMiddleware::new("conduit").using(TokenLocation::AuthorizationHeader).expiration_time(60));

    server.utilize(router! {
        get "/" => |request, response| {
            "<html><body><h1>Hello from <a href='https://github.com/hsharpsoftware/rust-realworld-example-app'>the test application written in Rust on Nickel</a> running in Azure Web App!</h1></body></html>"
        }
        get "/api/test1/:id" => |request, response| {      
            format!("This is test: {:?}", request.param("id"))
        }
        get "/api/test2/:id" => |request, response| {      
            // Get the objectId from the request params
            let object_id = request.param("id").unwrap();

            // Match the user id to an bson ObjectId
            let id = match ObjectId::with_string(object_id) {
                Ok(oid) => oid,
                Err(e) => return response.send(format!("{}", e))
            };

            format!("{}",id)
        }
    });

    if cfg!(db = "mongodb") {
        let client =
            Client::connect("localhost", 27017).expect("Failed to initialize standalone client.");

        let coll = client.db("test").collection("movies");

        let doc = doc! { "title" => "Jaws",
                      "array" => [ 1, 2, 3 ] };

        // Insert document into 'test.movies' collection
        coll.insert_one(doc.clone(), None)
            .ok()
            .expect("Failed to insert document.");

        // Find the document and receive a cursor
        let mut cursor = coll.find(Some(doc.clone()), None)
            .ok()
            .expect("Failed to execute find.");

        let item = cursor.next();

        // cursor.next() returns an Option<Result<Document>>
        match item {
            Some(Ok(doc)) => {
                match doc.get("title") {
                    Some(&Bson::String(ref title)) => println!("{}", title),
                    _ => panic!("Expected title to be a string!"),
                }
            }
            Some(Err(_)) => panic!("Failed to get next from server!"),
            None => panic!("Server returned no results!"),
        }
    }

    let port = iis::get_port();

    let listen_on = format!("127.0.0.1:{}", port);

    println!("Listening on {}", listen_on);

    server.listen(listen_on).unwrap();
}
