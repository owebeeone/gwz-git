//! The Git CLI route of `commit`, `tag_create` and `tag_delete`: porcelain
//! `git`, so hooks, signing and committer configuration are honored. The
//! module above picks it when the route rule says libgit2 cannot reproduce
//! Git exactly. Moved from gwz-core's `repository.rs` and `refs.rs`; the
//! child is launched through the caller's [`ProcessGate`](super::ProcessGate).

use std::path::Path;
use std::process::Command;

use crate::error::Error;
use crate::object_id::ObjectId;

use super::verify;
use super::{CommitOutcome, Services, TagOutcome};

/// Run `command` through the caller's gate and require success.
fn run(services: &Services<'_>, command: &mut Command, what: &str) -> Result<(), Error> {
    let output = services
        .process
        .output(command)
        .map_err(|err| Error::operation(format!("failed to run {what}: {err}")))?;
    if !output.status.success() {
        return Err(Error::operation(format!(
            "{what} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

pub(super) fn commit(
    services: &Services<'_>,
    path: &Path,
    message: &str,
    all: bool,
) -> Result<CommitOutcome, Error> {
    let before = verify::head(path)?;
    let mut command = Command::new("git");
    // The child's directory is explicit: it never inherits the host's.
    command.current_dir(path).arg("-C").arg(path).arg("commit");
    if all {
        command.arg("-a");
    }
    command.arg("-m").arg(message);
    run(services, &mut command, "git commit")?;
    // AD1 self-verify: HEAD advanced to a new commit (read fresh).
    let after: ObjectId =
        verify::head(path)?.ok_or_else(|| Error::operation("HEAD is unborn after commit"))?;
    if Some(after) == before {
        return Err(Error::operation("git commit did not advance HEAD"));
    }
    Ok(CommitOutcome { id: after })
}

pub(super) fn tag_create(
    services: &Services<'_>,
    path: &Path,
    name: &str,
    message: Option<&str>,
    signed: bool,
) -> Result<TagOutcome, Error> {
    let mut command = Command::new("git");
    // The child's directory is explicit: it never inherits the host's.
    command.current_dir(path).arg("-C").arg(path).arg("tag");
    if signed {
        command.arg("-s");
    } else if message.is_some() {
        command.arg("-a");
    }
    if let Some(message) = message {
        command.arg("-m").arg(message);
    }
    command.arg(name);
    run(services, &mut command, "git tag")?;
    verify::tag_created(path, name)
}

pub(super) fn tag_delete(services: &Services<'_>, path: &Path, name: &str) -> Result<(), Error> {
    let mut command = Command::new("git");
    command
        .current_dir(path)
        .arg("-C")
        .arg(path)
        .arg("tag")
        .arg("-d")
        .arg(name);
    run(services, &mut command, "git tag -d")?;
    verify::tag_deleted(path, name)
}
