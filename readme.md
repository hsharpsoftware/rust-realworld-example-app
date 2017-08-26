# ![RealWorld Example App](logo.png)

[![Build status](https://ci.appveyor.com/api/projects/status/8s17p2vh2f4e8a2y?svg=true)](https://ci.appveyor.com/project/davidpodhola/rust-realworld-example-app)

> ### Rust codebase containing real world examples (CRUD, auth, advanced patterns, etc) that adheres to the [RealWorld](https://github.com/gothinkster/realworld-example-apps) spec and API.


### [RealWorld](https://github.com/gothinkster/realworld)


This codebase was created to demonstrate a fully fledged fullstack application built with [Rust fast HTTP implementation Hyper](https://hyper.rs/) in including CRUD operations, authentication, routing, pagination, and more.

We've gone to great lengths to adhere to the [Rust community styleguides & best practices](https://aturon.github.io/README.html).

For more information on how to this works with other frontends/backends, head over to the [RealWorld](https://github.com/gothinkster/realworld) repo.


# How it works

This is an application written in [Rust](https://www.rust-lang.org/en-US/index.html) using these crates:

- [Hyper](https://hyper.rs/) - a fast HTTP implementation written in and for Rust
- [Tiberius](https://github.com/steffengy/tiberius) - Microsoft SQL Server async Rust driver written in Rust
- [Serde](https://serde.rs/) - a framework for serializing and deserializing Rust data structures efficiently and generically
- [Reroute](https://github.com/gsquire/reroute) - A router for Rust's hyper framework using regular expressions
- [IIS](https://github.com/hsharpsoftware/rust-web-iis) - Set of helper functions for running web server written in Rust on Internet Information Services (IIS) 

# Getting started

Install Rust: [https://www.rustup.rs/](https://www.rustup.rs/)

Get [Microsoft SQL Server **2017+**](https://www.microsoft.com/en-us/sql-server/sql-server-2017). SQL Express Edition is OK (when released), Azure SQL Database is OK, LocalDB does **NOT** work. Make sure TCP is enabled on the Server (enabled in Azure SQL by default, disabled by default on local installations).

On the desired Microsoft SQL Server run `database.sql` script to create database `Conduit` and all the tables, functions etc.

Copy `conduit - sample.toml` to `conduit.toml` and set your connection string there. Please note the connection encryption must adhere to crate configuration in Cargo.toml see [Tiberius documentation on Encryption](https://github.com/steffengy/tiberius#encryption-tlsssl). Default Cargo.toml configuration works for Azure SQL ([encrypted](https://docs.microsoft.com/en-us/azure/sql-database/sql-database-security-overview); if using please make sure you add your local IP address to the firewall rules).

Build locally with integration tests:

- `./locbld.cmd`

Or run locally:
- `./run.cmd`

API URL: `http://localhost:6767`
