use std::collections::HashMap;
use std::fmt::Display;
use std::path::{Path, PathBuf};

use anyhow::Result;
use etcetera::BaseStrategy;
use rusqlite::Connection;
use thiserror::Error;
use url::Url;

use crate::config::{read_manifest, ManifestHook, RepoLocation, RepoWire, MANIFEST_FILE};
use crate::fs::LockedFile;

// TODO: define errors
#[derive(Debug, Error)]
pub enum Error {}

#[derive(Debug)]
pub struct Repo {
    path: PathBuf,
    name: String,
    rev: String,
    hooks: HashMap<String, ManifestHook>,
}

impl Repo {
    pub fn from_path(name: String, rev: String, path: PathBuf) -> Result<Self> {
        let path = path.join(MANIFEST_FILE);
        let manifest = read_manifest(&path)?;
        let hooks = manifest
            .hooks
            .into_iter()
            .map(|hook| (hook.id.clone(), hook))
            .collect();

        Ok(Self {
            path,
            name,
            rev,
            hooks,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn rev(&self) -> &str {
        &self.rev
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn hooks(&self) -> &HashMap<String, ManifestHook> {
        &self.hooks
    }
}

impl Display for Repo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.name, self.rev)
    }
}

pub struct Store {
    path: PathBuf,
    conn: Option<Connection>,
}

impl Store {
    pub fn from_settings() -> Result<Self> {
        if let Some(path) = std::env::var_os("PRE_COMMIT_HOME") {
            Self::from_path(path)
        } else if let Ok(cache_dir) = etcetera::choose_base_strategy() {
            Self::from_path(cache_dir.cache_dir().join("pre-commit"))
        } else {
            Err(anyhow::anyhow!("Could not determine cache directory"))
        }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            conn: None,
        })
    }

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
            .map(|(name, rev, path)| Repo::from_path(name, rev, PathBuf::from(path)))
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

    pub fn init_repo(&self, repo: &RepoWire, deps: Option<Vec<String>>) -> Result<Repo> {
        match &repo.repo {
            RepoLocation::Remote(url) => self.init_remote_repo(repo, url, deps),
            RepoLocation::Local => self.init_local_repo(repo),
            RepoLocation::Meta => self.init_meta_repo(repo),
        }
    }

    fn init_remote_repo(
        &self,
        repo: &RepoWire,
        url: &Url,
        deps: Option<Vec<String>>,
    ) -> Result<Repo> {
        let _lock = self.lock()?;

        let repo_name = Self::repo_name(url.as_str(), deps.as_ref());
        let rev = repo.rev.as_ref().unwrap();

        let conn = self.conn.as_ref().unwrap();
        let mut stmt =
            conn.prepare("SELECT repo, ref, path FROM repos WHERE repo = ? AND ref = ?")?;
        let mut rows = stmt.query([repo_name.as_str(), &rev])?;
        if let Some(row) = rows.next()? {
            let path: String = row.get(2)?;
            return Ok(Repo::from_path(
                row.get(0)?,
                row.get(1)?,
                PathBuf::from(path),
            )?);
        }

        // TODO: 临时文件 persist
        // Clone and checkout the
        let temp = tempfile::Builder::new()
            .prefix("repo")
            .keep(true)
            .tempdir_in(&self.path)?;
        let path = temp.path().to_string_lossy().to_string();

        let mut stmt = self
            .conn
            .as_ref()
            .unwrap()
            .prepare("INSERT INTO repos (repo, ref, path) VALUES (?, ?, ?)")?;
        stmt.execute([repo_name.as_str(), rev, &path])?;

        Repo::from_path(repo_name, rev.clone(), PathBuf::from(path))
    }

    fn init_local_repo(&self, _repo: &RepoWire) -> Result<Repo> {
        todo!()
    }

    fn init_meta_repo(&self, _repo: &RepoWire) -> Result<Repo> {
        todo!()
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
