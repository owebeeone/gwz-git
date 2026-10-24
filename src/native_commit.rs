//! `commit`, `tag_create` and `tag_delete` over libgit2, with the Git CLI
//! kept for what libgit2 cannot reproduce.
//!
//! Operator decision 2026-10-12 (GwzNoFallbackPlan.md §6): these three use
//! libgit2 when nothing needs the Git CLI, and the CLI otherwise. `route`
//! holds the rule; this file picks the route and runs it. Removing the CLI
//! route altogether (hooks, signing, filters) is lane 3's later work.
//!
//! Operator decision 2026-10-13: `commit -a` always runs the Git CLI, as in
//! 1.0.17. The native `-a` staging (`staging` and the `Staging` steps of
//! `exec`) is kept in the tree for later; only a [`RoutePolicy`] with
//! `native_all` reaches it, and no production path sets that.
//!
//! These moved here from gwz-core in step 5.2 of the transport convergence
//! plan (operator decisions D-G and D-K; the 2026-10-14 "stays a module of
//! gwz-core" line is superseded). The caller gives the capture of the process
//! environment, the process gate the Git CLI is spawned under and the reader
//! of the "why Git ran" notes, in [`Services`]; this crate holds no global.

use std::io;
use std::path::Path;
use std::process::{Command, Output};

use crate::error::Error;
use crate::object_id::ObjectId;

mod cli;
mod config_view;
mod environment;
mod exec;
mod hooks;
mod ident;
mod message;
mod notes;
mod route;
mod staging;
mod system_config;
mod verify;

pub use environment::GitEnvironment;
pub use notes::RouteNotes;
use route::CliReason;

cfg_if::cfg_if! {
    if #[cfg(test)] {
        mod tests;
    }
}

/// Which route may be taken. The default is the route rule. Forcing the Git
/// CLI to compare it with the native route is a test facility: it exists only
/// under the `test-support` feature (and in this crate's own tests), so a
/// production consumer cannot force a route.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoutePolicy {
    cli_only: bool,
    native_all: bool,
}

impl RoutePolicy {
    /// The route rule decides.
    pub const AUTO: Self = Self {
        cli_only: false,
        native_all: false,
    };
}

cfg_if::cfg_if! {
    if #[cfg(any(test, feature = "test-support"))] {
        impl RoutePolicy {
            /// Always the Git CLI.
            pub const CLI_ONLY: Self = Self {
                cli_only: true,
                native_all: false,
            };
        }

        impl Services<'_> {
            /// A copy of these services under another route policy.
            pub fn with_policy(mut self, policy: RoutePolicy) -> Self {
                self.policy = policy;
                self
            }
        }
    }
}

/// The runner of the Git CLI route's child processes. The caller owns the
/// policy of how a child is launched (gwz-core holds its spawn gate across the
/// spawn, never across the wait); this crate builds the command and reads the
/// result. `output` has the semantics of [`Command::output`]: the child's
/// input is null, its output and diagnostics are piped, and it is waited for.
pub trait ProcessGate {
    fn output(&self, command: &mut Command) -> io::Result<Output>;
}

/// What a commit or tag call needs from its caller.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct Services<'a> {
    /// The captured `GIT_*` variables, `EMAIL`, `PATH` and `PROGRAMDATA`.
    pub environment: &'a GitEnvironment,
    /// The route policy; [`RoutePolicy::AUTO`] unless the `test-support`
    /// feature (or this crate's tests) says otherwise.
    pub(crate) policy: RoutePolicy,
    /// Where the "why Git ran" lines go, if the caller has a reader.
    pub notes: Option<&'a RouteNotes>,
    /// How the Git CLI route's child is launched.
    pub process: &'a dyn ProcessGate,
}

impl<'a> Services<'a> {
    pub fn new(environment: &'a GitEnvironment, process: &'a dyn ProcessGate) -> Self {
        Self {
            environment,
            policy: RoutePolicy::AUTO,
            notes: None,
            process,
        }
    }

    pub fn with_notes(mut self, notes: Option<&'a RouteNotes>) -> Self {
        self.notes = notes;
        self
    }

    fn plan_commit(
        &self,
        path: &Path,
        message: &str,
        all: bool,
    ) -> Result<(git2::Repository, route::CommitPlan), CliReason> {
        self.require_auto()?;
        let repo = route::open(path)?;
        let plan = route::plan_commit(
            self.environment,
            &repo,
            message,
            all,
            self.policy.native_all,
        )?;
        Ok((repo, plan))
    }

    fn plan_tag_create(
        &self,
        path: &Path,
        name: &str,
        message: Option<&str>,
        signed: bool,
    ) -> Result<(git2::Repository, route::TagPlan), CliReason> {
        self.require_auto()?;
        let repo = route::open(path)?;
        let plan = route::plan_tag_create(self.environment, &repo, name, message, signed)?;
        Ok((repo, plan))
    }

    fn plan_tag_delete(&self, path: &Path, name: &str) -> Result<git2::Repository, CliReason> {
        self.require_auto()?;
        let repo = route::open(path)?;
        route::plan_tag_delete(self.environment, &repo, name)?;
        Ok(repo)
    }

    /// Tell the caller's reader, if it gave one, why `operation` ran Git.
    fn tell_cli(&self, operation: &str, path: &Path, reason: &CliReason) {
        if let Some(notes) = self.notes {
            notes.cli_used(operation, path, reason);
        }
    }

    fn require_auto(&self) -> Result<(), CliReason> {
        if self.policy.cli_only {
            Err(CliReason::Forced)
        } else {
            Ok(())
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(test)] {
        impl RoutePolicy {
            /// The route rule, with the kept native `commit -a`.
            pub(crate) const NATIVE_ALL: Self = Self { cli_only: false, native_all: true };
        }

        /// The route one operation takes.
        #[derive(Clone, Debug, PartialEq, Eq)]
        enum Route {
            Native,
            Cli(CliReason),
        }

        impl Services<'_> {
            /// The route `commit` takes for this repository now.
            fn commit_route(&self, path: &Path, message: &str, all: bool) -> Route {
                match self.plan_commit(path, message, all) {
                    Ok(_) => Route::Native,
                    Err(reason) => Route::Cli(reason),
                }
            }

            /// The route `tag_create` takes for this repository now.
            fn tag_create_route(
                &self,
                path: &Path,
                name: &str,
                message: Option<&str>,
                signed: bool,
            ) -> Route {
                match self.plan_tag_create(path, name, message, signed) {
                    Ok(_) => Route::Native,
                    Err(reason) => Route::Cli(reason),
                }
            }

            /// The route `tag_delete` takes for this repository now.
            fn tag_delete_route(&self, path: &Path, name: &str) -> Route {
                match self.plan_tag_delete(path, name) {
                    Ok(_) => Route::Native,
                    Err(reason) => Route::Cli(reason),
                }
            }
        }
    }
}

/// The result of a commit: the id of the new commit.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct CommitOutcome {
    pub id: ObjectId,
}

/// The result of a tag creation: the name and the commit the tag resolves to.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct TagOutcome {
    pub name: String,
    pub commit: ObjectId,
}

/// Commit the index (`all` is `git commit -a`) of the repository at `path`.
pub fn commit(
    services: &Services<'_>,
    path: &Path,
    message: &str,
    all: bool,
) -> Result<CommitOutcome, Error> {
    let (repo, plan) = match services.plan_commit(path, message, all) {
        Ok(planned) => planned,
        Err(reason) => {
            services.tell_cli("commit", path, &reason);
            return cli::commit(services, path, message, all);
        }
    };
    let oid = exec::commit(&repo, plan)?;
    // AD1 self-verify: HEAD is the new commit (read fresh).
    let after = verify::head_after(path, "commit")?;
    let id = ObjectId::from_git2(oid);
    if after != Some(id) {
        return Err(Error::operation("HEAD is not the new commit after commit"));
    }
    Ok(CommitOutcome { id })
}

/// Create the tag `name` at HEAD; annotated when `message` is given.
pub fn tag_create(
    services: &Services<'_>,
    path: &Path,
    name: &str,
    message: Option<&str>,
    signed: bool,
) -> Result<TagOutcome, Error> {
    let (repo, plan) = match services.plan_tag_create(path, name, message, signed) {
        Ok(planned) => planned,
        Err(reason) => {
            services.tell_cli("tag", path, &reason);
            return cli::tag_create(services, path, name, message, signed);
        }
    };
    exec::tag_create(&repo, name, plan)?;
    verify::tag_created(path, name)
}

/// Delete the tag `name`.
pub fn tag_delete(services: &Services<'_>, path: &Path, name: &str) -> Result<(), Error> {
    let repo = match services.plan_tag_delete(path, name) {
        Ok(repo) => repo,
        Err(reason) => {
            services.tell_cli("tag delete", path, &reason);
            return cli::tag_delete(services, path, name);
        }
    };
    exec::tag_delete(&repo, name)?;
    verify::tag_deleted(path, name)
}
