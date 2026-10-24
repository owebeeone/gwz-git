//! The index as an owned snapshot, moved from gwz-core's
//! `Git2Backend::repository_index` by step 5.5 (the tail of row G2).

use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::object_id::ObjectId;
use crate::repository::Repository;

/// One index entry: the lossless facts the index file holds for a path.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct IndexEntry {
    /// The path as the index stores it, in bytes (not necessarily UTF-8).
    pub path: Vec<u8>,
    /// The object the entry names, in the repository's object format.
    pub id: ObjectId,
    pub mode: u32,
    /// The entry flags, including the stage bits (a conflict stage is 1 to 3).
    pub flags: u16,
    pub flags_extended: u16,
    /// Seconds and nanoseconds of the change time the index recorded.
    pub ctime: (i32, u32),
    /// Seconds and nanoseconds of the modification time the index recorded.
    pub mtime: (i32, u32),
    /// The stat words the index recorded: device, inode, user, group, size.
    pub stat: [u32; 5],
}

/// The index of a repository at one moment, owning its data.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct IndexSnapshot {
    /// The index file's path, when the index has one.
    pub path: Option<PathBuf>,
    /// Every entry, in the index's order (by path, then stage).
    pub entries: Vec<IndexEntry>,
}

impl Repository {
    /// Reads the index and returns it as an owned snapshot. A bare repository
    /// has no entries (libgit2 gives it an empty index). A failure to read the
    /// index file is a `StateRead` error with the native diagnostics.
    pub fn index_snapshot(&self) -> Result<IndexSnapshot, Error> {
        let index = self.native.index().map_err(Error::state_read_native)?;
        Ok(IndexSnapshot {
            path: index.path().map(Path::to_path_buf),
            entries: index
                .iter()
                .map(|entry| IndexEntry {
                    path: entry.path,
                    id: ObjectId::from_git2(entry.id),
                    mode: entry.mode,
                    flags: entry.flags,
                    flags_extended: entry.flags_extended,
                    ctime: (entry.ctime.seconds(), entry.ctime.nanoseconds()),
                    mtime: (entry.mtime.seconds(), entry.mtime.nanoseconds()),
                    stat: [entry.dev, entry.ino, entry.uid, entry.gid, entry.file_size],
                })
                .collect(),
        })
    }
}
