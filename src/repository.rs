use std::ffi::OsStr;
use std::path::Path;

use crate::commit::{self, CommitRecord};
use crate::error::{Error, ErrorKind};
use crate::object_id::{ObjectFormat, ObjectId};

pub struct Repository {
    native: git2::Repository,
}

impl Repository {
    pub fn open_exact(path: &Path) -> Result<Self, Error> {
        let native = git2::Repository::open_ext(
            path,
            git2::RepositoryOpenFlags::NO_SEARCH,
            &[] as &[&OsStr],
        )
        .map_err(|error| Error::from_native(ErrorKind::RepositoryOpen, error))?;
        Ok(Self { native })
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
