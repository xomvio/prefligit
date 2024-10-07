use std::path::Path;
use rusqlite::Connection;

pub struct Store {
    conn: Connection,
}

pub struct Repo {
    pub name: String,
    pub r#ref: String,
    pub path: String,
}

impl Store {
    pub fn from_path(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        Ok(Self { conn })
    }

    pub fn repos(&self) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT repo FROM repos")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut repos = Vec::new();
        for repo in rows {
            repos.push(repo?);
        }
        Ok(repos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_store() -> Result<()> {
        let db_path = Path::new("/Users/Jo/.cache/pre-commit/db.db");
        let store = Store::from_path(&db_path)?;
        let repos = store.repos()?;
        assert_eq!(repos, vec!["bar", "qux"]);

        Ok(())
    }
}
