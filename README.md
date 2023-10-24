# PostgreSQL Database Migrator

A simple database migration library that will read SQL files from a directory
in order and apply them to a PostgreSQL database.

## Usage

Make sure you have a database:

```
createdb migrations_development && \
  echo "DATABASE_URL=postgres://username@localhost/migrations_development" > .env
```

Point the migrator at your migrations directory and Postgres connection:

```rust
use std::path::Path;

use dotenv::dotenv;
use migrator::Migrator;
use postgres::{Client, NoTls};

fn main() {
    dotenv().ok();

    let migrations_path = Path::new(file!()).parent().unwrap();
    let migrations_path = migrations_path.join("db").join("migrations");

    let database_url = dotenv::var("DATABASE_URL").unwrap();

    let mut conn = Client::connect(&database_url, NoTls).unwrap(); // This can fail when db conn fails

    let migrator = Migrator::new(migrations_path);
    migrator.migrate(&mut conn);
}
```
