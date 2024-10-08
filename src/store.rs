use std::path::Path;
use rusqlite::Connection;

pub struct Store {
    conn: Connection,
}

#[derive(Debug)]
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

    pub fn repos(&self) -> rusqlite::Result<Vec<Repo>> {
        let mut stmt = self.conn.prepare("SELECT repo, ref, path FROM repos")?;
         let repos = stmt.query_map([], |row|
            Ok(Repo {
                name: row.get(0)?,
                r#ref: row.get(1)?,
                path: row.get(2)?,
            })
        )?.collect::<Result<Vec<_>, _>>();
        repos
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
        println!("{:#?}", repos);

        Ok(())
    }
}
