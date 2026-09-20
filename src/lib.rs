//! Read-only, single-repository Git foundations for GWZ.
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
//! deliberately neither cloneable nor shareable. The G0 surface performs no
//! process, network, environment, cwd, ref, index, or worktree mutation.
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
mod object_id;
mod repository;

pub use commit::{CommitRecord, Signature};
pub use error::{Error, ErrorKind, NativeDiagnostic};
pub use object_id::{ObjectFormat, ObjectId};
pub use repository::Repository;
