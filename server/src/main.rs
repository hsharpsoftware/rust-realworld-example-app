#[macro_use]
extern crate nickel;

#[macro_use(bson, doc)]
extern crate bson;
extern crate mongodb;

extern crate iis;
extern crate hyper;

extern crate nickel_jwt_session;

use nickel::{Nickel, HttpRouter, Request, Response, MiddlewareResult, NickelError};
use hyper::header::{AccessControlAllowOrigin, AccessControlAllowHeaders, Headers};
use nickel::status::StatusCode;

use nickel_jwt_session::{SessionMiddleware, TokenLocation};

use bson::oid::ObjectId;

fn enable_cors<'mw>(_req: &mut Request, mut res: Response<'mw>) -> MiddlewareResult<'mw> {
    // Set appropriate headers

    // see https://github.com/nickel-org/nickel.rs/issues/365#issuecomment-234772648
    res.headers_mut().set_raw("Access-Control-Allow-Origin", vec![b"*".to_vec()]);
    res.headers_mut().set_raw("Access-Control-Allow-Headers", vec![b"Origin X-Requested-With Content-Type Accept".to_vec()]);

    // Pass control to the next middleware
    res.next_middleware()
}

use bson::Bson;
use mongodb::{Client, ThreadedClient};
use mongodb::db::ThreadedDatabase;

fn main() {
    let mut server = Nickel::new();
    server.utilize(enable_cors);
    server.utilize(SessionMiddleware::new("conduit").using(TokenLocation::AuthorizationHeader).expiration_time(60));

    server.utilize(router! {
        get "/" => |request, response| {
            "Hello from the test application written in Rust on Nickel running in Azure Web App!"
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
