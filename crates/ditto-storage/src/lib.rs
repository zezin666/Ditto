//! SQLite storage boundary. The schema and actor implementation belong to T2.

use std::path::Path;

#[derive(Debug)]
pub struct Storage {
    connection: rusqlite::Connection,
}

impl Storage {
    pub fn open(path: &Path) -> ditto_model::Result<Self> {
        let connection = rusqlite::Connection::open(path).map_err(|error| {
            ditto_model::DittoError::Configuration(format!(
                "failed to open {}: {error}",
                path.display()
            ))
        })?;
        Ok(Self { connection })
    }

    pub fn user_version(&self) -> ditto_model::Result<u32> {
        self.connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| ditto_model::DittoError::Configuration(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_sqlite_has_fts5_trigram() {
        let connection = rusqlite::Connection::open_in_memory().expect("open in-memory SQLite");
        let mut statement = connection
            .prepare("PRAGMA compile_options")
            .expect("prepare compile options");
        let options = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query compile options")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect compile options");
        assert!(
            options.iter().any(|option| option == "ENABLE_FTS5"),
            "bundled SQLite lacks ENABLE_FTS5: {options:?}"
        );
        connection
            .execute_batch("CREATE VIRTUAL TABLE probe USING fts5(text, tokenize = 'trigram');")
            .expect("bundled SQLite lacks the FTS5 trigram tokenizer");
    }
}
