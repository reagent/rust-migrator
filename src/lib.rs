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

    pub fn migrate(&self, conn: &mut Client) -> Result<(), postgres::Error> {
        let table = MigrationsTable::new("migrations");
        table.create(conn)?;

        for file in self.files() {
            if !table.was_migration_applied(conn, &file)? {
                table.apply_migration(conn, &file)?;
            }
        }

        Ok(())
    }

    fn files(&self) -> Vec<MigrationFile> {
        let mut migration_files: Vec<MigrationFile> = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Some(file) = MigrationFile::from(entry) {
                        migration_files.push(file)
                    }
                }
            }
        }

        migration_files.sort();

        migration_files
    }
}

#[derive(Eq, Debug)]
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

    pub fn was_migration_applied(
        &self,
        conn: &mut Client,
        file: &MigrationFile,
    ) -> Result<bool, postgres::Error> {
        let query = format!(
            "SELECT COUNT(id) AS count FROM {table_name} WHERE id = $1",
            table_name = self.table_name
        );

        let result = conn.query_one(&query, &[&file.id])?;
        let count: i64 = result.get("count");

        Ok(count != 0)
    }

    pub fn apply_migration(
        &self,
        conn: &mut Client,
        file: &MigrationFile,
    ) -> Result<(), postgres::Error> {
        let mut transaction = conn.transaction()?;

        let stmt = file
            .contents()
            .expect(&format!("Could not read contents of file: {:?}", file));

        transaction.batch_execute(&stmt)?;

        let stmt = format!(
            "INSERT INTO {table_name} (id) VALUES ($1)",
            table_name = self.table_name
        );

        transaction.execute(&stmt, &[&file.id])?;
        transaction.commit()?;

        Ok(())
    }
}

impl MigrationFile {
    fn from(possible: DirEntry) -> Option<MigrationFile> {
        if let Ok(metadata) = possible.metadata() {
            if !metadata.is_file() {
                return None;
            }

            let pattern = Regex::new(r"^(\d{3})_[^.]+\.sql$").unwrap();

            let base_path = possible.path().parent().unwrap().to_path_buf();
            let filename = possible.file_name();
            let filename = filename.to_str().unwrap();

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

    pub fn contents(&self) -> Result<String, std::io::Error> {
        let path = Path::new(&self.base_path).join(&self.filename);

        let mut contents = String::new();
        let mut file = File::open(&path)?;

        file.read_to_string(&mut contents)?;

        Ok(contents)
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
