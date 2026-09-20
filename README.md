# gwz-git

`gwz-git` is an unpublished single-repository Rust foundation for GWZ, not yet
connected to the production core or CLI. G0 opens an
explicit local repository with the qualified `git2-rs`/libgit2 1.9.7 source and
reads one full-ID commit into owned records.

The foundation performs no Git executable lookup, subprocess, network access,
credential prompt, lazy fetch, ref/index/worktree mutation, current-directory
change, or process-environment mutation. `Repository` owns one native handle;
move it between workers for sequential use and do not share it concurrently.

```rust,no_run
use gwz_git::{ObjectId, Repository};

fn inspect(path: &std::path::Path, full_id: &str) -> Result<(), gwz_git::Error> {
    let repository = Repository::open_exact(path)?;
    let id = ObjectId::parse_hex(repository.object_format(), full_id)?;
    let commit = repository.read_commit(id)?;
    println!("{} has {} parents", commit.id, commit.parents.len());
    Ok(())
}
```

The [API guide](../gwz-core/dev-docs/GwzGitLibraryApi.md) specifies exact opening,
ID formats, record fields, errors and lifetimes. Drop releases the native handle;
returned commit records remain usable. Paths may identify a worktree root, Git
directory or bare root. Nested paths do not search parents. Reads do not lock
against other processes or provide a multi-read snapshot.

From this prepared workspace, format and test with the locked Rust 1.95
toolchain and isolated target directory:

```text
cd /path/to/gwz-dev/gwz-git
CARGO_TARGET_DIR=/tmp/gwz-git-g0-target rustup run 1.95.0 \
  cargo fmt --check
CARGO_TARGET_DIR=/tmp/gwz-git-g0-target rustup run 1.95.0 \
  cargo test --locked
CARGO_TARGET_DIR=/tmp/gwz-git-g0-target rustup run 1.95.0 \
  cargo clippy --locked --all-targets -- -D warnings
```

The current package is prepared-workspace-only and unpublished. It requires the
qualified Rust fork at `../git2-rs` commit
`ce78628308e11b4e8901d5061602619109bce21a` and vendored libgit2 1.9.7 source
at `b172e3d187a4b6866fd9f696f40a1b8e7f56d348`. From `gwz-core`, the source
qualification proof is:

```text
cd /path/to/gwz-dev/gwz-core
python3 tests/transport_native/prove.py --git2-source ../git2-rs
```

The dependency source, nested native checkout, lockfile, and production
activation require the separate integrator qualification gates. G0 does not implement commit,
tag, history, fetch, hook, signer, or generic process-runner APIs. Later
commit/tag work may run configured hook and signer helper programs, but it must
never use a Git executable fallback. Remove the isolated build output when it is
no longer needed with `rm -rf /tmp/gwz-git-g0-target`.
