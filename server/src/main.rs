#[macro_use]
extern crate nickel;

#[macro_use(bson, doc)]
extern crate bson;
extern crate mongodb;

use nickel::Nickel;

use bson::Bson;
use mongodb::{Client, ThreadedClient};
use mongodb::db::ThreadedDatabase;

fn main() {
    let mut server = Nickel::new();

    server.utilize(router! {
        get "**" => |_req, _res| {
            "Hello world!"
        }
    });

    let client =
        Client::connect("localhost", 27017).expect("Failed to initialize standalone client.");

    let coll = client.db("test").collection("movies");

    server.listen("127.0.0.1:6767");
}
