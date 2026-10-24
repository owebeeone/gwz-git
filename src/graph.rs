//! Commit graph reads, moved from gwz-core's `Git2Backend::is_ancestor`,
//! `ahead_behind`, `merge_base` and `changed_paths_between` by step 5.3 (row
//! G2).

use crate::error::Error;
use crate::object_id::ObjectId;
use crate::repository::Repository;

/// How far one commit is from another: the commits reachable from one and not
/// the other. Both counts are zero when the two are the same commit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct AheadBehind {
    pub ahead: u64,
    pub behind: u64,
}

impl Repository {
    /// Whether `ancestor` is a proper ancestor of `descendant` (a commit is not
    /// its own ancestor).
    pub fn is_ancestor(&self, ancestor: ObjectId, descendant: ObjectId) -> Result<bool, Error> {
        self.require_format(ancestor)?;
        self.require_format(descendant)?;
        self.native
            .graph_descendant_of(descendant.to_git2(), ancestor.to_git2())
            .map_err(Error::object_read)
    }

    /// The commits reachable from `local` and not from `upstream` (ahead) and
    /// the other way round (behind).
    pub fn ahead_behind(&self, local: ObjectId, upstream: ObjectId) -> Result<AheadBehind, Error> {
        self.require_format(local)?;
        self.require_format(upstream)?;
        let (ahead, behind) = self
            .native
            .graph_ahead_behind(local.to_git2(), upstream.to_git2())
            .map_err(Error::object_read)?;
        Ok(AheadBehind {
            ahead: ahead as u64,
            behind: behind as u64,
        })
    }

    /// The best common ancestor of two commits; `None` when their histories
    /// share none.
    pub fn merge_base(&self, left: ObjectId, right: ObjectId) -> Result<Option<ObjectId>, Error> {
        self.require_format(left)?;
        self.require_format(right)?;
        match self.native.merge_base(left.to_git2(), right.to_git2()) {
            Ok(base) => Ok(Some(ObjectId::from_git2(base))),
            Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(error) => Err(Error::object_read(error)),
        }
    }

    /// Every path that differs between the trees of two commits, old and new
    /// side of each delta, sorted and without repeats (so a rename lists both
    /// its paths). Names that are not UTF-8 are converted lossily.
    pub fn changed_paths(&self, old: ObjectId, new: ObjectId) -> Result<Vec<String>, Error> {
        self.require_format(old)?;
        self.require_format(new)?;
        let old = self
            .native
            .find_commit(old.to_git2())
            .map_err(Error::object_read)?;
        let new = self
            .native
            .find_commit(new.to_git2())
            .map_err(Error::object_read)?;
        let old_tree = old.tree().map_err(Error::object_read)?;
        let new_tree = new.tree().map_err(Error::object_read)?;
        let diff = self
            .native
            .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)
            .map_err(Error::object_read)?;
        let mut paths = Vec::new();
        for delta in diff.deltas() {
            for file in [delta.old_file(), delta.new_file()] {
                if let Some(path) = file.path() {
                    paths.push(path.to_string_lossy().into_owned());
                }
            }
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
