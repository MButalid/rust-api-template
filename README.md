### Code Tour

If you're familiar with SQL, I recommend starting in the `migrations/` directory, as that's what contains the SQL files
that define the schema of the PostgreSQL database, which is usually the first thing done when prototyping a new project. 
The files there are filled with comments explaining the various decisions made while structuring the database,
as well as advice on good practices for schema architecture and how this compares to the Realworld spec.

Next, of course, is `main.rs` as the entrypoint for the application. It shows the typical boilerplate that goes
into spinning up a Rust backend application.

I then recommend going to `lib.rs` and recursively exploring the modules as they're defined.
Comments on the module definitions will guide you from there.

[`clap`]: https://github.com/clap-rs/clap/

## Setup

### Clone this Repository

```shell
$ git clone https://github.com/MButalid/rust-api-template
$ cd rust-api-template
```
