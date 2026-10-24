//! The vocabulary these tests were written in while they lived in gwz-core
//! (`Git2Backend`, `ModelResult`, `ErrorCode`), as a thin test fixture over
//! this crate's `native_commit` API, so the moved tests assert what they
//! asserted there. Nothing here is production code: the fixture reads
//! repositories with `git2` directly, as core's backend did.

use std::io;
use std::path::Path;
use std::process::{Command, Output};

use crate::error::Error;
use crate::native_commit::{self, GitEnvironment, ProcessGate, RoutePolicy, Services};
use crate::repository::Repository;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ErrorCode {
    GitCommandFailed,
}

#[derive(Debug)]
#[allow(
    dead_code,
    reason = "the message is read through Debug output in failures"
)]
pub(super) struct ModelError {
    pub(super) code: ErrorCode,
    pub(super) message: String,
}

pub(super) type ModelResult<T> = Result<T, ModelError>;

pub(super) fn model_error(error: Error) -> ModelError {
    ModelError {
        code: ErrorCode::GitCommandFailed,
        message: error.message().to_owned(),
    }
}

fn native_error(error: git2::Error) -> ModelError {
    ModelError {
        code: ErrorCode::GitCommandFailed,
        message: error.message().to_owned(),
    }
}

#[derive(Debug)]
pub(super) struct GitCommitResult {
    pub(super) commit: String,
}

#[derive(Debug)]
#[allow(
    dead_code,
    reason = "the name is read through Debug output in failures"
)]
pub(super) struct GitTagResult {
    pub(super) name: String,
    pub(super) commit: String,
}

pub(super) use crate::native_commit::RouteNotes;

pub(super) struct GitHeadState {
    pub(super) commit: Option<String>,
}

/// Spawns with `Command::output`, which is what core's gate does on a platform
/// whose gate does not exclude.
struct DirectProcess;

impl ProcessGate for DirectProcess {
    fn output(&self, command: &mut Command) -> io::Result<Output> {
        command.output()
    }
}

/// The fixture "backend": an environment capture, a route policy and a reader
/// of the route notes, as core's `Git2Backend` held them.
#[derive(Clone)]
pub(super) struct Git2Backend {
    pub(super) environment: GitEnvironment,
    policy: RoutePolicy,
    notes: Option<RouteNotes>,
}

impl Git2Backend {
    pub(super) fn new() -> Self {
        Self {
            environment: GitEnvironment::from_vars(std::env::vars_os()),
            policy: RoutePolicy::AUTO,
            notes: None,
        }
    }

    pub(super) fn with_route_policy(&self, policy: RoutePolicy) -> Self {
        let mut backend = self.clone();
        backend.policy = policy;
        backend
    }

    pub(super) fn with_environment(&self, environment: GitEnvironment) -> Self {
        let mut backend = self.clone();
        backend.environment = environment;
        backend
    }

    pub(super) fn with_route_notes(&self, notes: RouteNotes) -> Self {
        let mut backend = self.clone();
        backend.notes = Some(notes);
        backend
    }

    fn with_services<T>(&self, call: impl FnOnce(&Services<'_>) -> T) -> T {
        let process = DirectProcess;
        let services = Services::new(&self.environment, &process)
            .with_policy(self.policy)
            .with_notes(self.notes.as_ref());
        call(&services)
    }

    pub(super) fn commit(
        &self,
        path: &Path,
        message: &str,
        all: bool,
    ) -> ModelResult<GitCommitResult> {
        self.with_services(|services| native_commit::commit(services, path, message, all))
            .map(|outcome| GitCommitResult {
                commit: outcome.id.to_string(),
            })
            .map_err(model_error)
    }

    pub(super) fn tag_create(
        &self,
        path: &Path,
        name: &str,
        message: Option<&str>,
        signed: bool,
    ) -> ModelResult<GitTagResult> {
        self.with_services(|services| {
            native_commit::tag_create(services, path, name, message, signed)
        })
        .map(|outcome| GitTagResult {
            name: outcome.name,
            commit: outcome.commit.to_string(),
        })
        .map_err(model_error)
    }

    pub(super) fn tag_delete(&self, path: &Path, name: &str) -> ModelResult<()> {
        self.with_services(|services| native_commit::tag_delete(services, path, name))
            .map_err(model_error)
    }

    pub(super) fn commit_route(
        &self,
        path: &Path,
        message: &str,
        all: bool,
    ) -> native_commit::Route {
        self.with_services(|services| services.commit_route(path, message, all))
    }

    pub(super) fn tag_create_route(
        &self,
        path: &Path,
        name: &str,
        message: Option<&str>,
        signed: bool,
    ) -> native_commit::Route {
        self.with_services(|services| services.tag_create_route(path, name, message, signed))
    }

    pub(super) fn tag_delete_route(&self, path: &Path, name: &str) -> native_commit::Route {
        self.with_services(|services| services.tag_delete_route(path, name))
    }

    /// A repository born the way gwz births one: `main`, and the filter
    /// pins core's `create_repo` writes at creation.
    pub(super) fn create_repo(&self, path: &Path) -> ModelResult<()> {
        let mut options = git2::RepositoryInitOptions::new();
        options.bare(false).no_reinit(true).initial_head("main");
        let repo = git2::Repository::init_opts(path, &options).map_err(native_error)?;
        let mut config = repo.config().map_err(native_error)?;
        config
            .set_bool("core.autocrlf", false)
            .map_err(native_error)?;
        config.set_str("core.eol", "lf").map_err(native_error)?;
        Ok(())
    }

    pub(super) fn stage_paths(&self, path: &Path, pathspecs: &[&str]) -> ModelResult<()> {
        let repo = git2::Repository::open(path).map_err(native_error)?;
        let mut index = repo.index().map_err(native_error)?;
        index
            .add_all(
                pathspecs.iter().copied(),
                git2::IndexAddOption::DEFAULT,
                None,
            )
            .map_err(native_error)?;
        index.write().map_err(native_error)
    }

    pub(super) fn head(&self, path: &Path) -> ModelResult<GitHeadState> {
        let commit = Repository::open_exact(path)
            .and_then(|repo| repo.resolve("HEAD"))
            .map_err(model_error)?;
        Ok(GitHeadState {
            commit: commit.map(|id| id.to_string()),
        })
    }
}

// Only the Unix-only characterization tests read the status and the tag list.
cfg_if::cfg_if! {
    if #[cfg(unix)] {
        pub(super) struct GitStatus {
            pub(super) untracked: usize,
        }

        impl Git2Backend {
            pub(super) fn status(&self, path: &Path) -> ModelResult<GitStatus> {
                let repo = git2::Repository::open(path).map_err(native_error)?;
                let mut options = git2::StatusOptions::new();
                options.include_untracked(true).recurse_untracked_dirs(true);
                let statuses = repo.statuses(Some(&mut options)).map_err(native_error)?;
                let untracked = statuses
                    .iter()
                    .filter(|entry| entry.status().contains(git2::Status::WT_NEW))
                    .count();
                Ok(GitStatus { untracked })
            }

            pub(super) fn tag_list(&self, path: &Path) -> ModelResult<Vec<String>> {
                let repo = git2::Repository::open(path).map_err(native_error)?;
                let names = repo.tag_names(None).map_err(native_error)?;
                let mut tags = Vec::new();
                for entry in names.iter() {
                    if let Some(name) = entry.map_err(native_error)? {
                        tags.push(name.to_owned());
                    }
                }
                tags.sort();
                Ok(tags)
            }
        }
    }
}
