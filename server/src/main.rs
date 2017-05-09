#[macro_use]
extern crate nickel;

#[macro_use(bson, doc)]
extern crate bson;
extern crate mongodb;

extern crate iis;
extern crate hyper;

use nickel::{Nickel, HttpRouter, Request, Response, MiddlewareResult};
use hyper::header::{AccessControlAllowOrigin, AccessControlAllowHeaders};

fn enable_cors<'mw>(_req: &mut Request, mut res: Response<'mw>) -> MiddlewareResult<'mw> {
    // Set appropriate headers
    res.set(AccessControlAllowOrigin::Any);
    res.set(AccessControlAllowHeaders(vec![
        // Hyper uses the `unicase::Unicase` type to ensure comparisons are done
        // case-insensitively. Here, we use `into()` to convert to one from a `&str`
        // so that we don't have to import the type ourselves.
        "Origin".into(),
        "X-Requested-With".into(),
        "Content-Type".into(),
        "Accept".into(),
    ]));

    // Pass control to the next middleware
    res.next_middleware()
}

use bson::Bson;
use mongodb::{Client, ThreadedClient};
use mongodb::db::ThreadedDatabase;

fn main() {
    let mut server = Nickel::new();
    server.utilize(enable_cors);

    server.utilize(router! {
        get "**" => |_req, _res| {
            "Hello from the test application written in Rust on Nickel running in Azure Web App!"
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

    server.listen(listen_on);
}
