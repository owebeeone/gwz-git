//! Single-repository Git behavior for GWZ: read-only foundations (open, ids,
//! commit records, revision resolve), since step 5.2 native commit and tag in
//! [`native_commit`], and since step 5.3 the refs and reads of a repository
//! (`head`, `status`, tag names, the commit graph, committed files, remotes),
//! and since step 5.5 the index as an owned snapshot.
//!
//! ```rust,no_run
//! use gwz_git::{ObjectId, Repository};
//!
//! fn inspect(path: &std::path::Path, full_id: &str) -> Result<(), gwz_git::Error> {
//!     let repository = Repository::open_exact(path)?;
//!     let id = ObjectId::parse_hex(repository.object_format(), full_id)?;
//!     let commit = repository.read_commit(id)?;
//!     println!("{} has {} parents", commit.id, commit.parents.len());
//!     Ok(())
//! }
//! ```
//!
//! `Repository` owns one native handle and may move between workers, but it is
//! deliberately neither cloneable nor shareable. The G0 read surface performs no
//! process, network, environment, cwd, ref, index, or worktree mutation; the
//! `native_commit` calls change refs and the index, and may launch the Git CLI
//! through a caller-supplied [`native_commit::ProcessGate`].
//! [`Repository::open_from_env`] is the one open that lets the native layer
//! read Git's variables from the process environment; it still changes none.
//! `native_commit` opens through it; gwz-core has no caller of its own.
//!
//! ```rust,compile_fail
//! fn requires_sync<T: Sync>() {}
//! fn main() { requires_sync::<gwz_git::Repository>(); }
//! ```
//!
//! ```rust,compile_fail
//! fn requires_clone<T: Clone>() {}
//! fn main() { requires_clone::<gwz_git::Repository>(); }
//! ```

//! Error values intentionally do not promise cloning or equality.
//!
//! ```rust,compile_fail
//! fn requires_clone<T: Clone>() {}
//! requires_clone::<gwz_git::Error>();
//! ```
//!
//! ```rust,compile_fail
//! fn requires_eq<T: PartialEq>() {}
//! requires_eq::<gwz_git::Error>();
//! ```
//!
//! ```rust,compile_fail
//! fn requires_clone<T: Clone>() {}
//! requires_clone::<gwz_git::NativeDiagnostic>();
//! ```
//!
//! ```rust,compile_fail
//! fn requires_eq<T: PartialEq>() {}
//! requires_eq::<gwz_git::NativeDiagnostic>();
//! ```

mod commit;
mod error;
mod graph;
mod index;
pub mod native_commit;
mod object_id;
mod objects;
mod refs;
mod remotes;
mod repository;
mod status;

pub use commit::{CommitRecord, Signature};
pub use error::{Error, ErrorKind, NativeDiagnostic};
pub use graph::AheadBehind;
pub use index::{IndexEntry, IndexSnapshot};
pub use object_id::{ObjectFormat, ObjectId};
pub use refs::Head;
pub use remotes::Remote;
pub use repository::Repository;
pub use status::{FileStatus, Status, StatusOptions};
