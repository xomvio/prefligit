use std::path::{Path, PathBuf};

use anyhow::Result;
use etcetera::BaseStrategy;
use rusqlite::Connection;
use thiserror::Error;
use tracing::{debug, trace};

use crate::config::{ConfigLocalRepo, ConfigRemoteRepo, ConfigRepo};
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
            debug!(
                "Loading store from PRE_COMMIT_HOME: {}",
                path.to_string_lossy()
            );
            return Ok(Self::from_path(path));
        }
        let dirs = etcetera::choose_base_strategy()?;
        let path = dirs.cache_dir().join("pre-commit");
        debug!("Loading store from cache directory: {}", path.display());
        Ok(Self::from_path(path))
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

        // TODO: fix, local repo can also in the store
        let repos = rows
            .into_iter()
            .map(|(url, rev, path)| Repo::remote(&url, &rev, &path))
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

    fn get_repo(
        &self,
        repo: &str,
        rev: &str,
        deps: Option<&Vec<String>>,
    ) -> Result<Option<(String, String, String)>> {
        let repo_name = Self::repo_name(repo, deps);

        let conn = self.conn.as_ref().unwrap();
        let mut stmt =
            conn.prepare("SELECT repo, ref, path FROM repos WHERE repo = ? AND ref = ?")?;
        let mut rows = stmt.query([repo_name.as_str(), rev])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some((row.get(0)?, row.get(1)?, row.get(2)?)))
    }

    fn insert_repo(
        &self,
        repo: &str,
        rev: &str,
        path: &str,
        deps: Option<Vec<String>>,
    ) -> Result<()> {
        let repo_name = Self::repo_name(repo, deps.as_ref());

        let mut stmt = self
            .conn
            .as_ref()
            .unwrap()
            .prepare("INSERT INTO repos (repo, ref, path) VALUES (?, ?, ?)")?;
        stmt.execute([repo_name.as_str(), rev, path])?;
        Ok(())
    }

    pub async fn prepare_local_repo(
        &self,
        repo_config: &ConfigLocalRepo,
        deps: Option<Vec<String>>,
    ) -> Result<Repo> {
        const LOCAL_REV: &str = "1";

        let language = repo_config.language;

        let path = match self.get_repo(repo_config.repo.as_str(), LOCAL_REV, deps.as_ref())? {
            Some((_, _, path)) => path,
            None => {
                let _lock = self.lock()?;

                let temp = tempfile::Builder::new()
                    .prefix("repo")
                    .keep(true)
                    .tempdir_in(&self.path)?;
                let path = temp.path().to_string_lossy().to_string();

                debug!("Creating local repo at {}", path);
                make_local_repo(repo_config.repo.as_str(), temp.path())?;

                self.insert_repo(repo_config.repo.as_str(), LOCAL_REV, &path, deps)?;
                path
            }
        };

        Repo::local(repo_config.hooks.clone(), &path)
    }

    pub async fn prepare_remote_repo(
        &self,
        repo_config: &ConfigRemoteRepo,
        deps: Option<Vec<String>>,
    ) -> Result<Repo> {
        if let Some((name, rev, path)) = self.get_repo(
            repo_config.repo.as_str(),
            repo_config.rev.as_str(),
            deps.as_ref(),
        )? {
            return Ok(Repo::remote(&name, &rev, &path)?);
        }

        // Clone and checkout the repo.
        let temp = tempfile::Builder::new()
            .prefix("repo")
            .keep(true)
            .tempdir_in(&self.path)?;
        let path = temp.path().to_string_lossy().to_string();

        debug!(
            "Cloning {}@{} into {}",
            repo_config.repo, repo_config.rev, path
        );
        clone_repo(repo_config.repo.as_str(), &repo_config.rev, temp.path()).await?;

        self.insert_repo(
            repo_config.repo.as_str(),
            repo_config.rev.as_str(),
            &path,
            deps,
        )?;

        Repo::remote(repo_config.repo.as_str(), &repo_config.rev, &path)
    }

    /// Prepare a repo in the store.
    ///
    /// For remote repos, clone the repo and checkout the ref.
    /// For local repos that need a environment, create the environment.
    pub async fn prepare_repo(&self, repo_config: &ConfigRepo, deps: Option<Vec<String>>) -> Result<Repo> {
        match repo_config {
            ConfigRepo::Remote(repo) => self.prepare_remote_repo(repo, deps).await,
            ConfigRepo::Local(repo) => self.prepare_local_repo(repo, deps).await,
            ConfigRepo::Meta(_) => todo!(),
        }
    }

    /// Lock the store.
    pub fn lock(&self) -> Result<LockedFile, std::io::Error> {
        LockedFile::acquire_blocking(self.path.join(".lock"), "store")
    }
}

fn make_local_repo(_repo: &str, path: &Path) -> Result<()> {
    fs_err::create_dir_all(path)?;

    Ok(())
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
