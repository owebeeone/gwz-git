//! Worktree status, moved from gwz-core's `Git2Backend::status_with_options`
//! and its status-character helpers by step 5.3 (row G2).

use crate::error::Error;
use crate::repository::Repository;

/// What `status` reports beyond the default.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct StatusOptions {
    /// Also list and count ignored files. When false they are neither listed
    /// nor counted.
    pub include_ignored: bool,
}

impl StatusOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ignored(mut self, include: bool) -> Self {
        self.include_ignored = include;
        self
    }
}

/// One path's status. The characters are `git status --short`'s: the index
/// column is `A`, `M`, `D`, `R`, `T` or space; the worktree column is `?`, `!`,
/// `M`, `D`, `R`, `T` or space.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct FileStatus {
    pub path: String,
    pub index_status: char,
    pub worktree_status: char,
    /// Where a renamed path came from.
    pub original_path: Option<String>,
}

/// The counts and the files of a status. `files` lists the paths that are UTF-8
/// and have an index or worktree change; a path that is not UTF-8 is counted
/// but not listed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct Status {
    /// Anything staged, unstaged, untracked or unresolved.
    pub is_dirty: bool,
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub ignored: usize,
    pub unresolved: usize,
    pub files: Vec<FileStatus>,
}

fn staged_statuses() -> git2::Status {
    git2::Status::INDEX_NEW
        | git2::Status::INDEX_MODIFIED
        | git2::Status::INDEX_DELETED
        | git2::Status::INDEX_RENAMED
        | git2::Status::INDEX_TYPECHANGE
}

fn unstaged_statuses() -> git2::Status {
    git2::Status::WT_MODIFIED
        | git2::Status::WT_DELETED
        | git2::Status::WT_RENAMED
        | git2::Status::WT_TYPECHANGE
}

fn index_status_char(status: git2::Status) -> char {
    if status.contains(git2::Status::INDEX_NEW) {
        'A'
    } else if status.contains(git2::Status::INDEX_MODIFIED) {
        'M'
    } else if status.contains(git2::Status::INDEX_DELETED) {
        'D'
    } else if status.contains(git2::Status::INDEX_RENAMED) {
        'R'
    } else if status.contains(git2::Status::INDEX_TYPECHANGE) {
        'T'
    } else {
        ' '
    }
}

fn worktree_status_char(status: git2::Status) -> char {
    if status.contains(git2::Status::WT_NEW) {
        '?'
    } else if status.contains(git2::Status::IGNORED) {
        '!'
    } else if status.contains(git2::Status::WT_MODIFIED) {
        'M'
    } else if status.contains(git2::Status::WT_DELETED) {
        'D'
    } else if status.contains(git2::Status::WT_RENAMED) {
        'R'
    } else if status.contains(git2::Status::WT_TYPECHANGE) {
        'T'
    } else {
        ' '
    }
}

fn file_status(entry: &git2::StatusEntry<'_>) -> Option<FileStatus> {
    let status = entry.status();
    // git2 reports a rename entry under the OLD path; model it the way `git status` does:
    // current path = the new path, `original_path` = where it came from.
    let (path, original_path) = match rename_delta(entry, status) {
        Some((old, new)) => (new, Some(old)),
        None => (entry.path().ok()?.to_owned(), None),
    };
    Some(FileStatus {
        path,
        index_status: index_status_char(status),
        worktree_status: worktree_status_char(status),
        original_path,
    })
}

/// `(old_path, new_path)` when `entry` is a rename (staged or unstaged), else `None`.
fn rename_delta(entry: &git2::StatusEntry<'_>, status: git2::Status) -> Option<(String, String)> {
    if !status.intersects(git2::Status::INDEX_RENAMED | git2::Status::WT_RENAMED) {
        return None;
    }
    let delta = entry.head_to_index().or_else(|| entry.index_to_workdir())?;
    let old = delta.old_file().path()?.to_str()?.to_owned();
    let new = delta.new_file().path()?.to_str()?.to_owned();
    Some((old, new))
}

impl Repository {
    /// The worktree's status: staged, unstaged, untracked, ignored and
    /// conflicted paths, with renames detected between HEAD and the index and
    /// between the index and the worktree. A bare repository has no worktree
    /// and reports `StateRead`.
    pub fn status(&self, options: &StatusOptions) -> Result<Status, Error> {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true)
            .include_ignored(options.include_ignored)
            .recurse_untracked_dirs(true)
            // F17: detect renames so a `git mv` reports `R` + original_path
            // instead of an unrelated delete + add.
            .renames_head_to_index(true)
            .renames_index_to_workdir(true);
        let statuses = self
            .native
            .statuses(Some(&mut opts))
            .map_err(Error::state_read_native)?;
        let mut out = Status::default();
        for entry in statuses.iter() {
            let status = entry.status();
            if status.intersects(staged_statuses()) {
                out.staged += 1;
            }
            if status.intersects(unstaged_statuses()) {
                out.unstaged += 1;
            }
            if status.contains(git2::Status::WT_NEW) {
                out.untracked += 1;
            }
            if status.contains(git2::Status::IGNORED) {
                out.ignored += 1;
            }
            if status.contains(git2::Status::CONFLICTED) {
                out.unresolved += 1;
            }
            if let Some(file) = file_status(&entry) {
                out.files.push(file);
            }
        }
        out.is_dirty =
            out.staged > 0 || out.unstaged > 0 || out.untracked > 0 || out.unresolved > 0;
        Ok(out)
    }
}
