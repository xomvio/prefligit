use std::path::PathBuf;

use anyhow::Result;
use etcetera::BaseStrategy;
use rusqlite::Connection;
use thiserror::Error;
use tracing::trace;

use crate::config::ConfigRemoteRepo;
use crate::fs::LockedFile;
use crate::git::clone_repo;
use crate::hook::Repo;

// TODO: define errors
#[derive(Debug, Error)]
pub enum Error {}

pub struct Store {
    path: PathBuf,
    conn: Option<Connection>,
}

impl Store {
    pub fn from_settings() -> Result<Self> {
        if let Some(path) = std::env::var_os("PRE_COMMIT_HOME") {
            return Ok(Self::from_path(path));
        }
        let dirs = etcetera::choose_base_strategy()?;
        Ok(Self::from_path(dirs.cache_dir().join("pre-commit")))
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            conn: None,
        }
    }

    /// Initialize the store.
    pub fn init(self) -> Result<Self> {
        fs_err::create_dir_all(&self.path)?;

        // Write a README file.
        match fs_err::write(
            self.path.join("README"),
            b"This directory is maintained by the pre-commit project.\nLearn more: https://github.com/pre-commit/pre-commit\n",
        ) {
            Ok(_) => (),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(err) => return Err(err.into()),
        }

        let _lock = self.lock()?;

        // Init the database.
        let db = self.path.join("db.db");
        let conn = if !db.try_exists()? {
            trace!("Creating database: {}", db.display());
            let conn = Connection::open(&db)?;
            conn.execute(
                "CREATE TABLE repos (
                    repo TEXT NOT NULL,
                    ref TEXT NOT NULL,
                    path TEXT NOT NULL,
                    PRIMARY KEY (repo, ref)
                );",
                [],
            )?;
            conn
        } else {
            trace!("Opening database: {}", db.display());
            Connection::open(&db)?
        };

        Ok(Self {
            conn: Some(conn),
            ..self
        })
    }

    /// List all repos.
    pub fn repos(&self) -> Result<Vec<Repo>> {
        let mut stmt = self
            .conn
            .as_ref()
            .unwrap()
            .prepare("SELECT repo, ref, path FROM repos")?;

        let rows: Vec<_> = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let rev: String = row.get(1)?;
                let path: String = row.get(2)?;
                Ok((name, rev, path))
            })?
            .collect::<Result<_, _>>()?;

        let repos = rows
            .into_iter()
            .map(|(url, rev, path)| Repo::remote(url, rev, path))
            .collect::<Result<Vec<_>>>();

        repos
    }

    fn repo_name(repo: &str, deps: Option<&Vec<String>>) -> String {
        let mut name = repo.to_string();
        if let Some(deps) = deps {
            name.push_str(":");
            name.push_str(&deps.join(","));
        }
        name
    }

    pub fn clone_repo(&self, repo: &ConfigRemoteRepo, deps: Option<Vec<String>>) -> Result<Repo> {
        let _lock = self.lock()?;

        let repo_name = Self::repo_name(repo.repo.as_str(), deps.as_ref());

        let conn = self.conn.as_ref().unwrap();
        let mut stmt =
            conn.prepare("SELECT repo, ref, path FROM repos WHERE repo = ? AND ref = ?")?;
        let mut rows = stmt.query([repo_name.as_str(), &repo.rev])?;
        if let Some(row) = rows.next()? {
            return Ok(Repo::remote(row.get(0)?, row.get(1)?, row.get(2)?)?);
        }

        // Clone and checkout the repo.
        let temp = tempfile::Builder::new()
            .prefix("repo")
            .keep(true)
            .tempdir_in(&self.path)?;
        let path = temp.path().to_string_lossy().to_string();

        trace!("Cloning {}@{}", repo.repo, repo.rev);
        clone_repo(repo.repo.as_str(), &repo.rev, temp.path())?;

        let mut stmt = self
            .conn
            .as_ref()
            .unwrap()
            .prepare("INSERT INTO repos (repo, ref, path) VALUES (?, ?, ?)")?;
        stmt.execute([repo_name.as_str(), &repo.rev, &path])?;

        Repo::remote(repo_name, repo.rev.clone(), path)
    }

    /// Lock the store.
    pub fn lock(&self) -> Result<LockedFile, std::io::Error> {
        LockedFile::acquire_blocking(self.path.join(".lock"), "store")
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;

    #[test]
    fn test_store() -> Result<()> {
        let store = Store::from_settings()?.init()?;
        let repos = store.repos()?;
        println!("{:#?}", repos);

        Ok(())
    }
}
