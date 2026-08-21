# WIP site

This is the backend/frontend for the WIP site. 

## Config / Setup
The only things that must be setup are the database and the `.env` file. These can be done with these two 
consecutive commands (`sqlx` is required):
```sh
cp .env.default .env
sqlx migrate run
```

## Running
The server may be run with `cargo r`, where it will deploy the instance to `0.0.0.0:8001`.

### Release / Production
The production version should be compiled and then run as such:
```sh
cargo build --release
./target/release/wip-server
```