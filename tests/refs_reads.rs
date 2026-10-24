//! Step 5.3 (row G2, amendment A3): the refs and reads that gwz-core's
//! `Git2Backend` used to do itself. The first six tests are the core tests that
//! asserted these behaviours (`tests/g01.rs`, `g05.rs`, `g14.rs` of core's git
//! module), rewritten against this crate's API; the rest cover what the library
//! now owns for itself (typed ids, error kinds, the owned records).

use std::fs;
use std::path::{Path, PathBuf};

use gwz_git::{
    AheadBehind, ErrorKind, FileStatus, Head, ObjectFormat, ObjectId, Remote, Repository, Status,
    StatusOptions,
};
use tempfile::TempDir;

/// A repository the way gwz creates one: branch `main`, and the filter
/// neutralization pinned in its local configuration (gwz-core's `create_repo`)
/// so a checkout is blob-exact under any ambient `autocrlf`.
fn init(path: &Path, format: ObjectFormat) -> git2::Repository {
    let mut options = git2::RepositoryInitOptions::new();
    options.initial_head("main");
    options.object_format(match format {
        ObjectFormat::Sha1 => git2::ObjectFormat::Sha1,
        ObjectFormat::Sha256 => git2::ObjectFormat::Sha256,
        _ => panic!("unsupported object format in fixture"),
    });
    let repo = git2::Repository::init_opts(path, &options).unwrap();
    let mut config = repo.config().unwrap();
    config.set_bool("core.autocrlf", false).unwrap();
    config.set_str("core.eol", "lf").unwrap();
    drop(config);
    repo
}

struct Fixture {
    _temp: TempDir,
    path: PathBuf,
    native: git2::Repository,
}

fn fixture() -> Fixture {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("repo");
    let native = init(&path, ObjectFormat::Sha1);
    Fixture {
        _temp: temp,
        path,
        native,
    }
}

impl Fixture {
    fn open(&self) -> Repository {
        Repository::open_exact(&self.path).unwrap()
    }

    /// Writes `content` to `relative`, stages it and commits on HEAD (core's
    /// `commit_file`).
    fn commit_file(
        &self,
        relative: &str,
        content: &str,
        message: &str,
        parents: &[git2::Oid],
    ) -> git2::Oid {
        if let Some(parent) = Path::new(relative).parent() {
            fs::create_dir_all(self.path.join(parent)).unwrap();
        }
        fs::write(self.path.join(relative), content).unwrap();
        let mut index = self.native.index().unwrap();
        index.add_path(Path::new(relative)).unwrap();
        index.write().unwrap();
        let tree = self.native.find_tree(index.write_tree().unwrap()).unwrap();
        let signature = git2::Signature::now("GWZ Test", "gwz@example.invalid").unwrap();
        let parents = parents
            .iter()
            .map(|id| self.native.find_commit(*id).unwrap())
            .collect::<Vec<_>>();
        let parents = parents.iter().collect::<Vec<_>>();
        self.native
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .unwrap()
    }

    /// A commit that is not on HEAD: `parent`'s tree plus a top-level file,
    /// written without touching HEAD, the index or the worktree.
    fn commit_beside(
        &self,
        relative: &str,
        content: &str,
        message: &str,
        parent: git2::Oid,
    ) -> git2::Oid {
        let parent = self.native.find_commit(parent).unwrap();
        let mut builder = self
            .native
            .treebuilder(Some(&parent.tree().unwrap()))
            .unwrap();
        let blob = self.native.blob(content.as_bytes()).unwrap();
        builder.insert(relative, blob, 0o100644).unwrap();
        let tree = self.native.find_tree(builder.write().unwrap()).unwrap();
        let signature = git2::Signature::now("GWZ Test", "gwz@example.invalid").unwrap();
        self.native
            .commit(None, &signature, &signature, message, &tree, &[&parent])
            .unwrap()
    }

    fn stage(&self, relative: &str) {
        let mut index = self.native.index().unwrap();
        index.add_path(Path::new(relative)).unwrap();
        index.write().unwrap();
    }
}

fn id(oid: git2::Oid) -> ObjectId {
    ObjectId::parse_hex(ObjectFormat::Sha1, &oid.to_string()).unwrap()
}

fn status(repo: &Repository) -> Status {
    repo.status(&StatusOptions::new()).unwrap()
}

#[test]
fn an_empty_repository_reports_its_unborn_branch_without_a_commit() {
    let fixture = fixture();
    let repo = fixture.open();

    let head = repo.head().unwrap();

    assert_eq!(head.branch.as_deref(), Some("main"));
    assert_eq!(head.commit, None);
    assert!(!head.is_detached);
    assert_eq!(repo.resolve("HEAD").unwrap(), None);
}

#[test]
fn commit_exists_requires_a_local_object_that_peels_to_a_commit() {
    let fixture = fixture();
    let commit = fixture.commit_file("tracked.txt", "one", "initial", &[]);
    let tree = fixture.native.find_commit(commit).unwrap().tree_id();
    let repo = fixture.open();

    assert!(repo.commit_exists(id(commit)).unwrap());
    assert!(!repo.commit_exists(id(tree)).unwrap());
    let zeros = ObjectId::parse_hex(ObjectFormat::Sha1, &"0".repeat(40)).unwrap();
    assert!(!repo.commit_exists(zeros).unwrap());
    // Text that is not an id never reaches the repository.
    let error = ObjectId::parse_hex(ObjectFormat::Sha1, "not-an-oid").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidObjectId);
}

#[test]
fn commit_exists_peels_an_annotated_tag_to_its_commit() {
    let fixture = fixture();
    let commit = fixture.commit_file("tracked.txt", "one", "initial", &[]);
    let object = fixture.native.find_object(commit, None).unwrap();
    let signature = git2::Signature::now("GWZ Test", "gwz@example.invalid").unwrap();
    let tag = fixture
        .native
        .tag("v1", &object, &signature, "release", false)
        .unwrap();
    let repo = fixture.open();

    assert!(repo.commit_exists(id(tag)).unwrap());
}

#[test]
fn committed_file_reads_the_exact_tree_without_consulting_the_worktree() {
    let fixture = fixture();
    let first = fixture.commit_file("gwz.conf/gwz.yml", "version: one\n", "one", &[]);
    fs::write(fixture.path.join("gwz.conf/gwz.yml"), "version: two\n").unwrap();
    let repo = fixture.open();

    assert_eq!(
        repo.blob_at(id(first), "gwz.conf/gwz.yml").unwrap(),
        Some(b"version: one\n".to_vec())
    );
    assert_eq!(
        repo.blob_at(id(first), "gwz.conf/missing.yml").unwrap(),
        None
    );
    for refused in ["../outside", "/absolute", "a/../b", "./a"] {
        let error = repo.blob_at(id(first), refused).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidPath, "{refused:?}");
    }
}

#[test]
fn committed_path_that_is_not_a_file_is_an_error_not_a_miss() {
    let fixture = fixture();
    let first = fixture.commit_file("dir/file.txt", "x", "one", &[]);
    let repo = fixture.open();

    let error = repo.blob_at(id(first), "dir").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ObjectRead);
    assert!(
        error.message().contains("not a file"),
        "{}",
        error.message()
    );
}

#[test]
fn reports_commit_ancestry_without_moving_head() {
    let fixture = fixture();
    let first = fixture.commit_file("README.md", "one", "initial", &[]);
    let second = fixture.commit_file("README.md", "two", "second", &[first]);
    let repo = fixture.open();

    assert!(repo.is_ancestor(id(first), id(second)).unwrap());
    assert!(!repo.is_ancestor(id(second), id(first)).unwrap());
    // The same commit is not its own ancestor (libgit2's strict relation).
    assert!(!repo.is_ancestor(id(first), id(first)).unwrap());
    assert_eq!(repo.head().unwrap().commit, Some(id(second)));
}

/// GwzFetchPlan.md D6: `gwz fetch` reports `+A -B`, which `is_ancestor` cannot
/// answer. The counts are symmetric, zero for the same commit, and a linear
/// history reports one side only.
#[test]
fn counts_how_far_a_commit_is_ahead_of_and_behind_another() {
    let fixture = fixture();
    let base = fixture.commit_file("README.md", "one", "initial", &[]);
    let second = fixture.commit_file("README.md", "two", "second", &[base]);
    let third = fixture.commit_file("README.md", "three", "third", &[second]);
    let repo = fixture.open();

    let counts = |local: git2::Oid, upstream: git2::Oid| {
        let AheadBehind { ahead, behind, .. } = repo.ahead_behind(id(local), id(upstream)).unwrap();
        (ahead, behind)
    };
    assert_eq!(counts(base, base), (0, 0));
    assert_eq!(counts(third, base), (2, 0));
    assert_eq!(counts(base, third), (0, 2));

    // A divergence: a second child of the base, so each side has exactly one
    // commit the other does not.
    let sibling = fixture.commit_beside("OTHER.md", "side", "sibling", base);
    assert_eq!(counts(sibling, second), (1, 1));
}

#[test]
fn merge_base_names_the_common_ancestor_or_nothing() {
    let fixture = fixture();
    let base = fixture.commit_file("README.md", "one", "initial", &[]);
    let left = fixture.commit_file("README.md", "two", "left", &[base]);
    let right = fixture.commit_beside("OTHER.md", "side", "right", base);
    let repo = fixture.open();

    assert_eq!(
        repo.merge_base(id(left), id(right)).unwrap(),
        Some(id(base))
    );
    assert_eq!(repo.merge_base(id(base), id(left)).unwrap(), Some(id(base)));

    // A commit with no history in common: a root commit written directly.
    let tree = fixture
        .native
        .find_tree(fixture.native.treebuilder(None).unwrap().write().unwrap())
        .unwrap();
    let signature = git2::Signature::now("GWZ Test", "gwz@example.invalid").unwrap();
    let orphan = fixture
        .native
        .commit(None, &signature, &signature, "orphan", &tree, &[])
        .unwrap();
    assert_eq!(repo.merge_base(id(left), id(orphan)).unwrap(), None);
}

#[test]
fn changed_paths_lists_both_sides_of_every_delta_sorted_and_unique() {
    let fixture = fixture();
    let first = fixture.commit_file("old.txt", "stable contents\n", "seed", &[]);
    fixture.commit_file("keep.txt", "keep\n", "keep", &[first]);
    let second = fixture.native.head().unwrap().target().unwrap();
    fs::remove_file(fixture.path.join("old.txt")).unwrap();
    let mut index = fixture.native.index().unwrap();
    index.remove_path(Path::new("old.txt")).unwrap();
    index.write().unwrap();
    let third = fixture.commit_file("z/new.txt", "added\n", "change", &[second]);
    let repo = fixture.open();

    let paths = repo.changed_paths(id(first), id(third)).unwrap();
    assert_eq!(paths, vec!["keep.txt", "old.txt", "z/new.txt"]);
    assert!(repo.changed_paths(id(third), id(third)).unwrap().is_empty());
}

#[test]
fn head_follows_a_branch_and_a_detached_commit() {
    let fixture = fixture();
    let first = fixture.commit_file("a.txt", "a", "first", &[]);

    let on_branch = fixture.open().head().unwrap();
    assert_eq!(on_branch.branch.as_deref(), Some("main"));
    assert_eq!(on_branch.commit, Some(id(first)));
    assert!(!on_branch.is_detached);

    fixture.native.set_head_detached(first).unwrap();
    let detached: Head = fixture.open().head().unwrap();
    assert_eq!(detached.branch, None);
    assert_eq!(detached.commit, Some(id(first)));
    assert!(detached.is_detached);
}

#[test]
fn tag_names_are_sorted_and_include_annotated_tags() {
    let fixture = fixture();
    let commit = fixture.commit_file("a.txt", "a", "first", &[]);
    let repo = fixture.open();
    assert!(repo.tag_names().unwrap().is_empty());

    let object = fixture.native.find_object(commit, None).unwrap();
    let signature = git2::Signature::now("GWZ Test", "gwz@example.invalid").unwrap();
    fixture.native.tag_lightweight("b", &object, false).unwrap();
    fixture
        .native
        .tag("a", &object, &signature, "annotated", false)
        .unwrap();
    fixture.native.tag_lightweight("c", &object, false).unwrap();

    assert_eq!(repo.tag_names().unwrap(), vec!["a", "b", "c"]);
}

#[test]
fn status_reports_clean_untracked_unstaged_and_staged() {
    let fixture = fixture();
    fixture.commit_file("tracked.txt", "one", "initial", &[]);
    let repo = fixture.open();

    assert_eq!(status(&repo), Status::default());

    fs::write(fixture.path.join("untracked.txt"), "new").unwrap();
    let found = status(&repo);
    assert!(found.is_dirty);
    assert_eq!(found.untracked, 1);
    fs::remove_file(fixture.path.join("untracked.txt")).unwrap();

    fs::write(fixture.path.join("tracked.txt"), "two").unwrap();
    let found = status(&repo);
    assert!(found.is_dirty);
    assert_eq!((found.unstaged, found.staged), (1, 0));

    fixture.stage("tracked.txt");
    let found = status(&repo);
    assert!(found.is_dirty);
    assert_eq!((found.unstaged, found.staged), (0, 1));
}

#[test]
fn status_hides_ignored_files_unless_requested() {
    let fixture = fixture();
    fixture.commit_file(".gitignore", "ignored/\n", "ignore", &[]);
    fs::create_dir_all(fixture.path.join("ignored")).unwrap();
    fs::write(fixture.path.join("ignored/cache.txt"), "cache").unwrap();
    let repo = fixture.open();

    assert_eq!(status(&repo), Status::default());

    let with_ignored = repo
        .status(&StatusOptions::new().with_ignored(true))
        .unwrap();
    assert!(!with_ignored.is_dirty);
    assert_eq!(with_ignored.untracked, 0);
    assert_eq!(with_ignored.ignored, 1);
    assert!(
        with_ignored
            .files
            .iter()
            .any(|file| file_is(file, "ignored/", ' ', '!', None)),
        "{:?}",
        with_ignored.files
    );
}

fn file_is(
    file: &FileStatus,
    path: &str,
    index: char,
    worktree: char,
    original: Option<&str>,
) -> bool {
    file.path == path
        && file.index_status == index
        && file.worktree_status == worktree
        && file.original_path.as_deref() == original
}

#[test]
fn status_detects_a_staged_rename_with_its_original_path() {
    // F17: rename detection is on: a `git mv` is one `R` entry with the
    // original path, not an unrelated delete and add.
    let fixture = fixture();
    fixture.commit_file("old.txt", "stable contents\n", "seed", &[]);
    fs::rename(fixture.path.join("old.txt"), fixture.path.join("new.txt")).unwrap();
    let mut index = fixture.native.index().unwrap();
    index.remove_path(Path::new("old.txt")).unwrap();
    index.add_path(Path::new("new.txt")).unwrap();
    index.write().unwrap();
    let repo = fixture.open();

    let found = status(&repo);

    assert!(found.is_dirty);
    assert!(
        found
            .files
            .iter()
            .any(|file| file_is(file, "new.txt", 'R', ' ', Some("old.txt"))),
        "{:?}",
        found.files
    );
    assert!(found.files.iter().all(|file| file.path != "old.txt"));
}

#[test]
fn status_counts_a_conflicted_entry_as_unresolved_and_dirty() {
    let fixture = fixture();
    let base = fixture.commit_file("f.txt", "base\n", "base", &[]);
    let ours = fixture.commit_file("f.txt", "ours\n", "ours", &[base]);
    fixture
        .native
        .branch("theirs", &fixture.native.find_commit(base).unwrap(), false)
        .unwrap();
    fixture.native.set_head("refs/heads/theirs").unwrap();
    fixture
        .native
        .checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .unwrap();
    let theirs = fixture.commit_file("f.txt", "theirs\n", "theirs", &[base]);
    fixture.native.set_head("refs/heads/main").unwrap();
    fixture
        .native
        .checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .unwrap();
    assert_eq!(fixture.native.head().unwrap().target(), Some(ours));
    let annotated = fixture.native.find_annotated_commit(theirs).unwrap();
    fixture.native.merge(&[&annotated], None, None).unwrap();
    let repo = fixture.open();

    let found = status(&repo);

    assert!(found.is_dirty);
    assert_eq!(found.unresolved, 1);
}

#[test]
fn remotes_lists_names_urls_and_push_urls() {
    let fixture = fixture();
    let repo = fixture.open();
    assert!(repo.remotes().unwrap().is_empty());

    fixture
        .native
        .remote("origin", "file:///tmp/origin.git")
        .unwrap();
    fixture
        .native
        .remote_set_pushurl("origin", Some("file:///tmp/push.git"))
        .unwrap();
    fixture
        .native
        .remote("mirror", "file:///tmp/mirror.git")
        .unwrap();

    let mut found = repo.remotes().unwrap();
    found.sort_by(|left, right| left.name.cmp(&right.name));
    let summary: Vec<(&str, &str, Option<&str>)> = found
        .iter()
        .map(
            |Remote {
                 name,
                 url,
                 push_url,
                 ..
             }| (name.as_str(), url.as_str(), push_url.as_deref()),
        )
        .collect();
    assert_eq!(
        summary,
        vec![
            ("mirror", "file:///tmp/mirror.git", None),
            (
                "origin",
                "file:///tmp/origin.git",
                Some("file:///tmp/push.git")
            ),
        ]
    );
}

#[test]
fn an_id_of_the_other_object_format_is_refused_before_the_native_lookup() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("sha256");
    init(&path, ObjectFormat::Sha256);
    let repo = Repository::open_exact(&path).unwrap();
    let sha1_id = ObjectId::parse_hex(ObjectFormat::Sha1, &"1".repeat(40)).unwrap();

    assert_eq!(
        repo.commit_exists(sha1_id).unwrap_err().kind(),
        ErrorKind::ObjectFormatMismatch
    );
    assert_eq!(
        repo.blob_at(sha1_id, "a.txt").unwrap_err().kind(),
        ErrorKind::ObjectFormatMismatch
    );
    assert_eq!(
        repo.is_ancestor(sha1_id, sha1_id).unwrap_err().kind(),
        ErrorKind::ObjectFormatMismatch
    );
    assert_eq!(
        repo.ahead_behind(sha1_id, sha1_id).unwrap_err().kind(),
        ErrorKind::ObjectFormatMismatch
    );
    assert_eq!(
        repo.merge_base(sha1_id, sha1_id).unwrap_err().kind(),
        ErrorKind::ObjectFormatMismatch
    );
    assert_eq!(
        repo.changed_paths(sha1_id, sha1_id).unwrap_err().kind(),
        ErrorKind::ObjectFormatMismatch
    );
}

#[test]
fn reads_on_a_bare_repository_that_has_no_worktree_say_so() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("bare.git");
    git2::Repository::init_bare(&path).unwrap();
    let repo = Repository::open_exact(&path).unwrap();

    let error = repo.status(&StatusOptions::new()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::StateRead);
    assert!(repo.tag_names().unwrap().is_empty());
    assert!(repo.remotes().unwrap().is_empty());
}
