use std::{
    cmp::Ordering,
    fs::{self, DirEntry, File},
    io::Read,
    path::{Path, PathBuf},
};

use postgres::Client;
use regex::Regex;

pub struct Migrator {
    path: PathBuf,
}

impl Migrator {
    pub fn new(path: PathBuf) -> Migrator {
        Migrator { path }
    }

    // TODO: return Result
    pub fn migrate(&self, conn: &mut Client) {
        let table = MigrationsTable::new("migrations");
        table.create(conn).unwrap(); // TODO: handle

        for file in self.files() {
            if !table.was_migration_applied(conn, &file) {
                table.apply_migration(conn, &file);
            }
        }
    }

    fn files(&self) -> Vec<MigrationFile> {
        let mut migration_files: Vec<MigrationFile> = Vec::new();

        if let Ok(files) = fs::read_dir(&self.path) {
            for file in files {
                let file = file.unwrap();

                if let Some(file) = MigrationFile::from(file) {
                    migration_files.push(file)
                }
            }
        }

        migration_files.sort();

        migration_files
    }
}

#[derive(Eq)]
struct MigrationFile {
    base_path: PathBuf,
    filename: String,
    id: String,
}

struct MigrationsTable {
    table_name: String,
}

impl MigrationsTable {
    fn new(table_name: &str) -> MigrationsTable {
        MigrationsTable {
            table_name: String::from(table_name),
        }
    }

    fn create(&self, conn: &mut Client) -> Result<(), postgres::Error> {
        let stmt = format!(
            r#"
              CREATE TABLE IF NOT EXISTS "{table_name}" (
                id CHAR(3) NOT NULL PRIMARY KEY,
                created_at TIMESTAMP NOT NULL DEFAULT NOW()
              )
            "#,
            table_name = self.table_name
        );

        conn.batch_execute(&stmt)
    }

    pub fn was_migration_applied(&self, conn: &mut Client, file: &MigrationFile) -> bool {
        let query = format!(
            "SELECT COUNT(id) AS count FROM {table_name} WHERE id = $1",
            table_name = self.table_name
        );

        let result = conn.query_one(&query, &[&file.id]).unwrap(); // TODO
        let count: i64 = result.get("count");

        count != 0
    }

    pub fn apply_migration(&self, conn: &mut Client, file: &MigrationFile) {
        let mut transaction = conn.transaction().unwrap();
        transaction.batch_execute(&file.contents()).unwrap();

        let stmt = format!(
            "INSERT INTO {table_name} (id) VALUES ($1)",
            table_name = self.table_name
        );

        transaction.execute(&stmt, &[&file.id]).unwrap();
        transaction.commit().unwrap();
    }
}

impl MigrationFile {
    fn from(possible: DirEntry) -> Option<MigrationFile> {
        if let Ok(metadata) = possible.metadata() {
            if !metadata.is_file() {
                return None;
            }

            let base_path = possible.path().parent().unwrap().to_path_buf();
            let filename = possible.file_name();
            let filename = filename.to_str().unwrap();

            let pattern = Regex::new(r"^(\d{3})_[^.]+\.sql$").unwrap();

            if let Some(captures) = pattern.captures(filename) {
                let id = captures.get(1).unwrap().as_str();

                return Some(MigrationFile {
                    base_path,
                    filename: String::from(filename),
                    id: String::from(id),
                });
            }
        }

        None
    }

    pub fn contents(&self) -> String {
        let path = Path::new(&self.base_path).join(&self.filename);

        let mut contents = String::new();
        let mut file = File::open(&path).expect("Dude");

        file.read_to_string(&mut contents).expect("hwyyy");

        contents
    }
}

impl Ord for MigrationFile {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for MigrationFile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for MigrationFile {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
