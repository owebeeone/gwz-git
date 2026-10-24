//! HEAD and tag names: the reads of gwz-core's `Git2Backend::head` and
//! `tag_list`, moved here by step 5.3 (row G2).

use std::fs;

use crate::error::Error;
use crate::object_id::ObjectId;
use crate::repository::Repository;

/// What `HEAD` points at: the branch it is on, if any, and the commit.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Head {
    /// The short name of the branch `HEAD` is on; `None` when detached.
    pub branch: Option<String>,
    /// The commit `HEAD` resolves to; `None` while the branch is unborn.
    pub commit: Option<ObjectId>,
    pub is_detached: bool,
}

impl Repository {
    /// The state of `HEAD`. An unborn branch (no commit yet) is not an error:
    /// it reports the branch `HEAD` names and no commit.
    pub fn head(&self) -> Result<Head, Error> {
        let head = match self.native.head() {
            Ok(head) => head,
            Err(error) if error.code() == git2::ErrorCode::UnbornBranch => {
                return self.unborn_head();
            }
            Err(error) => return Err(Error::object_read(error)),
        };
        let branch = if head.is_branch() {
            Some(head.shorthand().map_err(Error::object_read)?.to_owned())
        } else {
            None
        };
        Ok(Head {
            branch,
            commit: head.target().map(ObjectId::from_git2),
            is_detached: !head.is_branch(),
        })
    }

    fn unborn_head(&self) -> Result<Head, Error> {
        let text = fs::read_to_string(self.native.path().join("HEAD"))
            .map_err(|error| Error::state_read(format!("cannot read HEAD: {error}")))?;
        let branch = text
            .trim()
            .strip_prefix("ref: refs/heads/")
            .map(ToOwned::to_owned);
        Ok(Head {
            branch,
            commit: None,
            is_detached: false,
        })
    }

    /// Whether the reference `refs/tags/{name}` exists. An exact reference
    /// lookup: no revision rules (a `git describe`-shaped name, a branch that
    /// shares the name) can stand in for the tag.
    pub(crate) fn tag_ref_exists(&self, name: &str) -> Result<bool, Error> {
        match self.native.find_reference(&format!("refs/tags/{name}")) {
            Ok(_) => Ok(true),
            Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(error) => Err(Error::object_read(error)),
        }
    }

    /// The commit the tag `name` peels to, by exact reference lookup;
    /// `Ok(None)` when `refs/tags/{name}` does not exist.
    pub(crate) fn tag_commit(&self, name: &str) -> Result<Option<ObjectId>, Error> {
        match self.native.find_reference(&format!("refs/tags/{name}")) {
            Ok(reference) => {
                let commit = reference.peel_to_commit().map_err(Error::object_read)?;
                Ok(Some(ObjectId::from_git2(commit.id())))
            }
            Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(error) => Err(Error::object_read(error)),
        }
    }

    /// The names of every tag (`v1`, not `refs/tags/v1`), sorted.
    pub fn tag_names(&self) -> Result<Vec<String>, Error> {
        let names = self.native.tag_names(None).map_err(Error::object_read)?;
        let mut tags = Vec::new();
        for entry in names.iter() {
            if let Some(name) = entry.map_err(Error::object_read)? {
                tags.push(name.to_owned());
            }
        }
        tags.sort();
        Ok(tags)
    }
}
