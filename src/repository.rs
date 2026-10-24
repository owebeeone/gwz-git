use std::ffi::OsStr;
use std::path::Path;

use crate::commit::{self, CommitRecord};
use crate::error::{Error, ErrorKind};
use crate::object_id::{ObjectFormat, ObjectId};

pub struct Repository {
    native: git2::Repository,
}

impl Repository {
    /// Opens exactly `path`, which is a worktree root, a Git directory or a
    /// bare root. Parents are not searched and no Git environment variable
    /// chooses anything.
    pub fn open_exact(path: &Path) -> Result<Self, Error> {
        Self::open_with(path, git2::RepositoryOpenFlags::NO_SEARCH)
    }

    /// Opens exactly `path`, as [`Repository::open_exact`] does, but lets Git's
    /// own environment variables shape the handle the way the `git` program
    /// reads them (`GIT_WORK_TREE`, `GIT_COMMON_DIR`, `GIT_OBJECT_DIRECTORY`,
    /// `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_INDEX_FILE`, `GIT_NAMESPACE`
    /// and the configuration locations). The native layer reads them from the
    /// process environment when the open runs; this crate reads and changes no
    /// environment itself. `GIT_DIR` does not choose the repository: with a
    /// path supplied, the path does.
    pub fn open_from_env(path: &Path) -> Result<Self, Error> {
        Self::open_with(
            path,
            git2::RepositoryOpenFlags::NO_SEARCH | git2::RepositoryOpenFlags::FROM_ENV,
        )
    }

    fn open_with(path: &Path, flags: git2::RepositoryOpenFlags) -> Result<Self, Error> {
        let native =
            git2::Repository::open_ext(path, flags, &[] as &[&OsStr]).map_err(Error::from_open)?;
        Ok(Self { native })
    }

    /// Resolves a revision expression (a ref name, a unique id prefix, `HEAD`,
    /// `name^{commit}`) to the id of the object it names. Nothing is peeled
    /// unless the revision says so: an annotated tag name yields the tag
    /// object. `Ok(None)` means the revision names nothing, including an
    /// unborn `HEAD`; an expression the native layer cannot parse or cannot
    /// decide (ambiguous) is an `ObjectRead` error.
    pub fn resolve(&self, revision: &str) -> Result<Option<ObjectId>, Error> {
        match self.native.revparse_single(revision) {
            Ok(object) => Ok(Some(ObjectId::from_git2(object.id()))),
            Err(error)
                if matches!(
                    error.code(),
                    git2::ErrorCode::NotFound | git2::ErrorCode::UnbornBranch
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(Error::from_native(ErrorKind::ObjectRead, error)),
        }
    }

    /// Whether `path` is a repository, opened as [`Repository::open_exact`]
    /// does (no parent search). A path that is not one, including a missing
    /// path, answers `Ok(false)`; a repository that is found and cannot be
    /// opened is an error.
    pub fn is_repository(path: &Path) -> Result<bool, Error> {
        match Self::open_exact(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == ErrorKind::NotARepository => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub fn git_dir(&self) -> &Path {
        self.native.path()
    }

    pub fn common_dir(&self) -> &Path {
        self.native.commondir()
    }

    pub fn work_dir(&self) -> Option<&Path> {
        self.native.workdir()
    }

    pub fn object_format(&self) -> ObjectFormat {
        match self.native.object_format() {
            git2::ObjectFormat::Sha1 => ObjectFormat::Sha1,
            git2::ObjectFormat::Sha256 => ObjectFormat::Sha256,
        }
    }

    pub fn read_commit(&self, id: ObjectId) -> Result<CommitRecord, Error> {
        if id.format() != self.object_format() {
            return Err(Error::format_mismatch(format!(
                "object id format {:?} does not match repository format {:?}",
                id.format(),
                self.object_format()
            )));
        }
        commit::read(&self.native, id)
    }
}
