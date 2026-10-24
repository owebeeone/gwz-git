# gwz-git

`gwz-git` is an unpublished single-repository Rust foundation for GWZ, not yet
connected to the production core or CLI. G0 opens an
explicit local repository with the qualified `git2-rs`/libgit2 1.9.7 source and
reads one full-ID commit into owned records.

The G0 read foundation performs no Git executable lookup, subprocess, network access,
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

Step 5.1b adds `Repository::open_from_env` (an open that lets Git's environment
variables shape the handle), `Repository::resolve` (a revision to an id) and
`ErrorKind::NotARepository`; the guide below carries the amendment.

Step 5.3 adds the refs and reads gwz-core used to do itself: `Repository::head`,
`tag_names`, `commit_exists`, `blob_at`, `is_ancestor`, `ahead_behind`,
`merge_base`, `changed_paths`, `status` and `remotes`, with the owned records
`Head`, `AheadBehind`, `Status`, `FileStatus`, `StatusOptions` and `Remote`
(guide section A3).

Step 5.5 adds `Repository::index_snapshot` with the owned records `IndexSnapshot` and `IndexEntry` (the index as gwz-core's
`repository_index` read it; guide section A3).

The [API guide](../gwz-core/dev-docs/GwzGitLibraryApi.md) specifies exact opening,
ID formats, record fields, errors and lifetimes. Drop releases the native handle;
returned commit records remain usable. Paths may identify a worktree root, Git
directory or bare root. Nested paths do not search parents. Reads do not lock
against other processes or provide a multi-read snapshot.

`scripts/process_globals_allowlist.json` is the crate's spawn and process-global
allowlist (empty: gwz-git spawns nothing). Check it with gwz-core's checker:
`python3.13 -B ../gwz-core/scripts/checks/check_process_globals.py --repo . --allowlist scripts/process_globals_allowlist.json`.
`.github/workflows/ci.yml` runs fmt, clippy and the tests on Windows. A Windows
host that must not use `D:/gwz-tests` sets `GWZ_GIT_TEST_TMP` (and `TEMP`) to a
scratch directory.

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
activation require the separate integrator qualification gates. Since step 5.2 the
crate also holds native commit and tag (`gwz_git::native_commit`, moved from
gwz-core): libgit2 when nothing needs the Git CLI, the CLI otherwise, launched
through the caller's `ProcessGate`; see the API guide's section A2. History,
fetch and a generic process-runner API are not provided. Remove the isolated build output when it is
no longer needed with `rm -rf /tmp/gwz-git-g0-target`.
