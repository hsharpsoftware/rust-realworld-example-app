set RUST_BACKTRACE=1
cargo run
IF DEFINED %APPVEYOR% ( exit 0 )

