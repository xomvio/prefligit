use std::path::PathBuf;

use anyhow::Result;
use etcetera::BaseStrategy;
use rusqlite::Connection;

use crate::fs::LockedFile;

#[derive(Debug)]
pub struct Repo {
    pub name: String,
    pub r#ref: String,
    pub path: String,
}

pub struct Store {
    path: PathBuf,
    conn: Connection,
}

impl Store {
    pub fn from_settings(path: Option<PathBuf>) -> Result<Self> {
        if let Some(path) = path {
            Self::from_path(path)
        } else if let Ok(cache_dir) = etcetera::choose_base_strategy() {
            Self::from_path(cache_dir.cache_dir().join("pre-commit").join("db.db"))
        } else {
            Err(anyhow::anyhow!("Could not determine cache directory"))
        }
    }

    pub fn from_path(path: PathBuf) -> Result<Self> {
        let conn = Connection::open(&path)?;
        Ok(Self { path, conn })
    }

    pub fn repos(&self) -> rusqlite::Result<Vec<Repo>> {
        let mut stmt = self.conn.prepare("SELECT repo, ref, path FROM repos")?;
        let repos = stmt
            .query_map([], |row| {
                Ok(Repo {
                    name: row.get(0)?,
                    r#ref: row.get(1)?,
                    path: row.get(2)?,
                })
            })?
            .collect();
        repos
    }

    pub fn lock(&self) -> Result<LockedFile, std::io::Error> {
        LockedFile::acquire_blocking(self.path.join(".lock"), "store")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_store() -> Result<()> {
        let store = Store::from_settings(None)?;
        let repos = store.repos()?;
        println!("{:#?}", repos);

        Ok(())
    }
}
