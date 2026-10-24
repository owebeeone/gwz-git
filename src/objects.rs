//! Object reads: commit existence and committed file content, moved from
//! gwz-core's `Git2Backend::commit_exists` and `read_file_at_commit` by step
//! 5.3 (row G2).

use std::path::{Component, Path};

use crate::error::Error;
use crate::object_id::ObjectId;
use crate::repository::Repository;

impl Repository {
    /// Whether `id` names an object in this repository that peels to a commit
    /// (a commit, or a tag of one). A missing object, or one of another type,
    /// is `Ok(false)`.
    pub fn commit_exists(&self, id: ObjectId) -> Result<bool, Error> {
        self.require_format(id)?;
        let object = match self.native.find_object(id.to_git2(), None) {
            Ok(object) => object,
            Err(error) if error.code() == git2::ErrorCode::NotFound => return Ok(false),
            Err(error) => return Err(Error::object_read(error)),
        };
        Ok(object.peel_to_commit().is_ok())
    }

    /// The content of the file at `path` in the tree of `commit`, exactly as
    /// committed (the worktree and index are not consulted). `path` is
    /// normalized and relative, with `/` separators: an absolute path, or one
    /// with a `.`, `..` or root component, is `InvalidPath`.
    /// `Ok(None)` when the commit's tree has no such path; a path that names a
    /// directory or a submodule is an `ObjectRead` error.
    pub fn blob_at(&self, commit: ObjectId, path: &str) -> Result<Option<Vec<u8>>, Error> {
        let relative = Path::new(path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(Error::invalid_path(format!(
                "committed file path must be normalized and relative: '{path}'"
            )));
        }
        self.require_format(commit)?;
        let commit = self
            .native
            .find_commit(commit.to_git2())
            .map_err(Error::object_read)?;
        let tree = commit.tree().map_err(Error::object_read)?;
        let entry = match tree.get_path(relative) {
            Ok(entry) => entry,
            Err(error) if error.code() == git2::ErrorCode::NotFound => return Ok(None),
            Err(error) => return Err(Error::object_read(error)),
        };
        let object = entry.to_object(&self.native).map_err(Error::object_read)?;
        let blob = object.as_blob().ok_or_else(|| {
            Error::object_read_plain(format!("committed path '{path}' is not a file"))
        })?;
        Ok(Some(blob.content().to_vec()))
    }
}
