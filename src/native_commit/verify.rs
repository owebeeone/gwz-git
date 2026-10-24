//! The AD1 self-verification reads around a commit or tag change. Each opens
//! the repository fresh, so what it sees is what is on disk.
//!
//! Tag existence is an exact reference lookup (`refs/tags/{name}`), never a
//! revision lookup: libgit2's revision rules would let a name shaped like
//! `git describe` output (`t-1-g<abbrev>`) or a branch called `refs/tags/X`
//! stand in for a tag that is not there. A read that fails after the write
//! it checks is an `Operation` error that says the write happened.

use std::path::Path;

use crate::error::Error;
use crate::object_id::ObjectId;
use crate::repository::Repository;

use super::TagOutcome;

/// HEAD's commit; `None` while the branch is unborn. A read before a write:
/// its failures keep their own kinds.
pub(super) fn head(path: &Path) -> Result<Option<ObjectId>, Error> {
    Repository::open_exact(path)?.resolve("HEAD")
}

/// HEAD's commit read back after `what` wrote; a failed read is
/// `Operation` ("<what>: written; verification read failed: ...").
pub(super) fn head_after(path: &Path, what: &str) -> Result<Option<ObjectId>, Error> {
    head(path).map_err(|error| Error::written_but_unverified(what, error))
}

/// The tag exists (read fresh) and resolves to a commit.
pub(super) fn tag_created(path: &Path, name: &str) -> Result<TagOutcome, Error> {
    let what = format!("tag '{name}' create");
    let unverified = |error| Error::written_but_unverified(&what, error);
    let commit = Repository::open_exact(path)
        .and_then(|repo| repo.tag_commit(name))
        .map_err(unverified)?
        .ok_or_else(|| Error::operation(format!("tag '{name}' missing after creation")))?;
    Ok(TagOutcome {
        name: name.to_owned(),
        commit,
    })
}

/// The tag is gone.
pub(super) fn tag_deleted(path: &Path, name: &str) -> Result<(), Error> {
    let what = format!("tag '{name}' delete");
    let present = Repository::open_exact(path)
        .and_then(|repo| repo.tag_ref_exists(name))
        .map_err(|error| Error::written_but_unverified(&what, error))?;
    if present {
        return Err(Error::operation(format!(
            "tag '{name}' still present after delete"
        )));
    }
    Ok(())
}
