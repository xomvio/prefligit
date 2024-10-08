use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use fs2::FileExt;
use tempfile::NamedTempFile;
use tracing::{debug, error, info, trace};

pub static CWD: LazyLock<PathBuf> =
    LazyLock::new(|| std::env::current_dir().expect("The current directory must be exist"));

/// A file lock that is automatically released when dropped.
#[derive(Debug)]
pub struct LockedFile(fs_err::File);

impl LockedFile {
    /// Inner implementation for [`LockedFile::acquire_blocking`] and [`LockedFile::acquire`].
    fn lock_file_blocking(file: fs_err::File, resource: &str) -> Result<Self, std::io::Error> {
        trace!(
            "Checking lock for `{resource}` at `{}`",
            file.path().display(),
        );
        match file.file().try_lock_exclusive() {
            Ok(()) => {
                debug!("Acquired lock for `{resource}`");
                Ok(Self(file))
            }
            Err(err) => {
                // Log error code and enum kind to help debugging more exotic failures
                if err.kind() != std::io::ErrorKind::WouldBlock {
                    trace!("Try lock error: {err:?}");
                }
                info!(
                    "Waiting to acquire lock for `{resource}` at `{}`",
                    file.path().display(),
                );
                file.file().lock_exclusive().map_err(|err| {
                    // Not an fs_err method, we need to build our own path context
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!(
                            "Could not acquire lock for `{resource}` at `{}`: {}",
                            file.path().display(),
                            err
                        ),
                    )
                })?;

                debug!("Acquired lock for `{resource}`");
                Ok(Self(file))
            }
        }
    }

    /// The same as [`LockedFile::acquire`], but for synchronous contexts. Do not use from an async
    /// context, as this can block the runtime while waiting for another process to release the
    /// lock.
    pub fn acquire_blocking(
        path: impl AsRef<Path>,
        resource: impl Display,
    ) -> Result<Self, std::io::Error> {
        let file = fs_err::File::create(path.as_ref())?;
        let resource = resource.to_string();
        Self::lock_file_blocking(file, &resource)
    }
}

impl Drop for LockedFile {
    fn drop(&mut self) {
        if let Err(err) = self.0.file().unlock() {
            error!(
                "Failed to unlock {}; program may be stuck: {}",
                self.0.path().display(),
                err
            );
        } else {
            debug!("Released lock at `{}`", self.0.path().display());
        }
    }
}

/// Return a [`NamedTempFile`] in the specified directory.
///
/// Sets the permissions of the temporary file to `0o666`, to match the non-temporary file default.
/// ([`NamedTempfile`] defaults to `0o600`.)
#[cfg(unix)]
pub fn tempfile_in(path: &Path) -> std::io::Result<NamedTempFile> {
    use std::os::unix::fs::PermissionsExt;
    tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o666))
        .tempfile_in(path)
}

/// Return a [`NamedTempFile`] in the specified directory.
#[cfg(not(unix))]
pub fn tempfile_in(path: &Path) -> std::io::Result<NamedTempFile> {
    tempfile::Builder::new().tempfile_in(path)
}

/// Write `data` to `path` atomically using a temporary file and atomic rename.
pub fn write_atomic(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> std::io::Result<()> {
    let temp_file = tempfile_in(
        path.as_ref()
            .parent()
            .expect("Write path must have a parent"),
    )?;
    fs_err::write(&temp_file, &data)?;
    temp_file.persist(&path).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "Failed to persist temporary file to {}: {}",
                path.as_ref().display(),
                err.error
            ),
        )
    })?;
    Ok(())
}

/// Copy `from` to `to` atomically using a temporary file and atomic rename.
pub fn copy_atomic(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<()> {
    let temp_file = tempfile_in(to.as_ref().parent().expect("Write path must have a parent"))?;
    fs_err::copy(from.as_ref(), &temp_file)?;
    temp_file.persist(&to).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "Failed to persist temporary file to {}: {}",
                to.as_ref().display(),
                err.error
            ),
        )
    })?;
    Ok(())
}
