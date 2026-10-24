//! The AD1 self-verification reads that follow a commit or tag change. Each
//! opens the repository fresh, so what it sees is what is on disk.

use std::path::Path;

use crate::error::Error;
use crate::object_id::ObjectId;
use crate::repository::Repository;

use super::TagOutcome;

/// HEAD's commit; `None` while the branch is unborn.
pub(super) fn head(path: &Path) -> Result<Option<ObjectId>, Error> {
    Repository::open_exact(path)?.resolve("HEAD")
}

fn tag_ref(name: &str) -> String {
    format!("refs/tags/{name}")
}

/// The tag exists (read fresh) and resolves to a commit.
pub(super) fn tag_created(path: &Path, name: &str) -> Result<TagOutcome, Error> {
    let repo = Repository::open_exact(path)?;
    if repo.resolve(&tag_ref(name))?.is_none() {
        return Err(Error::operation(format!(
            "tag '{name}' missing after creation"
        )));
    }
    let commit = repo
        .resolve(&format!("refs/tags/{name}^{{commit}}"))?
        .ok_or_else(|| Error::operation(format!("tag '{name}' did not resolve")))?;
    Ok(TagOutcome {
        name: name.to_owned(),
        commit,
    })
}

/// The tag is gone.
pub(super) fn tag_deleted(path: &Path, name: &str) -> Result<(), Error> {
    if Repository::open_exact(path)?
        .resolve(&tag_ref(name))?
        .is_some()
    {
        return Err(Error::operation(format!(
            "tag '{name}' still present after delete"
        )));
    }
    Ok(())
}
