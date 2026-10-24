//! Step 5.1b additions to the G0 surface: an environment-honouring open, a
//! revision `resolve`, and the `NotARepository` error kind.

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use gwz_git::{ErrorKind, ObjectFormat, ObjectId, Repository};
use tempfile::TempDir;

const CHILD_MARKER: &str = "GWZ_GIT_OPEN_RESOLVE_CHILD";
const CHILD_REPO: &str = "GWZ_GIT_OPEN_RESOLVE_REPO";
const CHILD_OTHER: &str = "GWZ_GIT_OPEN_RESOLVE_OTHER";

fn init_repo(path: &Path, format: ObjectFormat) -> git2::Repository {
    let mut options = git2::RepositoryInitOptions::new();
    options.initial_head("main");
    options.object_format(match format {
        ObjectFormat::Sha1 => git2::ObjectFormat::Sha1,
        ObjectFormat::Sha256 => git2::ObjectFormat::Sha256,
        _ => panic!("unsupported object format in fixture"),
    });
    git2::Repository::init_opts(path, &options).unwrap()
}

fn seed_commit(repo: &git2::Repository) -> git2::Oid {
    let signature = git2::Signature::new(
        "Seed",
        "seed@example.invalid",
        &git2::Time::new(1_700_000_000, 0),
    )
    .unwrap();
    let tree = repo
        .find_tree(repo.treebuilder(None).unwrap().write().unwrap())
        .unwrap();
    repo.commit(Some("HEAD"), &signature, &signature, "seed", &tree, &[])
        .unwrap()
}

/// Run the named test again in a child whose environment holds only `vars`.
/// Returns false in the child, which then performs the assertions.
fn in_clean_child(test_name: &str, repo: &Path, other: &Path, vars: &[(&str, &Path)]) -> bool {
    if env::var_os(CHILD_MARKER).is_some() {
        return false;
    }
    let mut command = Command::new(env::current_exe().unwrap());
    command.env_clear();
    if cfg!(windows) {
        for name in ["SystemRoot", "TEMP", "TMP"] {
            if let Some(value) = env::var_os(name) {
                command.env(name, value);
            }
        }
    }
    command
        .env(CHILD_MARKER, "1")
        .env(CHILD_REPO, repo)
        .env(CHILD_OTHER, other);
    for (name, value) in vars {
        command.env(name, value);
    }
    let output = command
        .args(["--exact", test_name, "--nocapture", "--test-threads", "1"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "child failed: {stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains(&format!("test {test_name} ... ok")),
        "child did not run the selected test: {stdout}"
    );
    true
}

fn two_repos() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    let other = temp.path().join("other");
    for path in [&repo, &other] {
        init_repo(path, ObjectFormat::Sha1);
    }
    (temp, repo, other)
}

#[test]
fn environment_open_honours_git_variables_that_exact_open_ignores() {
    let (_temp, repo, other) = two_repos();
    let other_worktree = other.join("elsewhere");
    fs::create_dir(&other_worktree).unwrap();
    let other_git = other.join(".git");
    let test = "environment_open_honours_git_variables_that_exact_open_ignores";
    if in_clean_child(
        test,
        &repo,
        &other,
        &[
            ("GIT_WORK_TREE", other_worktree.as_path()),
            ("GIT_DIR", other_git.as_path()),
        ],
    ) {
        return;
    }
    let repo = Path::new(&env::var_os(CHILD_REPO).unwrap()).to_path_buf();
    let other_worktree = Path::new(&env::var_os(CHILD_OTHER).unwrap()).join("elsewhere");

    let exact = Repository::open_exact(&repo).unwrap();
    let from_env = Repository::open_from_env(&repo).unwrap();

    // The exact open ignores every Git variable.
    assert!(exact.work_dir().unwrap().ends_with("repo"));
    // The environment open lets GIT_WORK_TREE choose the work tree.
    assert!(from_env.work_dir().unwrap().ends_with("elsewhere"));
    assert_ne!(exact.work_dir(), from_env.work_dir());
    assert_eq!(
        fs::canonicalize(from_env.work_dir().unwrap()).unwrap(),
        fs::canonicalize(&other_worktree).unwrap()
    );
    // With a path supplied, libgit2 consults GIT_DIR in neither open: the
    // path chooses the repository.
    assert_eq!(exact.git_dir(), from_env.git_dir());
    assert!(from_env.git_dir().ends_with(".git"));
    assert!(
        fs::canonicalize(from_env.git_dir())
            .unwrap()
            .starts_with(fs::canonicalize(&repo).unwrap())
    );
}

#[test]
fn environment_open_agrees_with_exact_open_in_a_clean_environment() {
    let (_temp, repo, other) = two_repos();
    let test = "environment_open_agrees_with_exact_open_in_a_clean_environment";
    if in_clean_child(test, &repo, &other, &[]) {
        return;
    }
    let repo = Path::new(&env::var_os(CHILD_REPO).unwrap()).to_path_buf();
    let exact = Repository::open_exact(&repo).unwrap();
    let from_env = Repository::open_from_env(&repo).unwrap();
    assert_eq!(exact.git_dir(), from_env.git_dir());
    assert_eq!(exact.common_dir(), from_env.common_dir());
    assert_eq!(exact.work_dir(), from_env.work_dir());
    assert_eq!(exact.object_format(), from_env.object_format());
    // Like the exact open, it does not search parents.
    let nested = repo.join("nested");
    fs::create_dir(&nested).unwrap();
    assert_eq!(
        Repository::open_from_env(&nested).err().unwrap().kind(),
        ErrorKind::NotARepository
    );
}

#[test]
fn resolve_names_refs_short_shas_and_missing_revisions() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("repo");
    let native = init_repo(&path, ObjectFormat::Sha1);
    let oid = seed_commit(&native);
    let commit = native.find_commit(oid).unwrap();
    native
        .branch("topic", &commit, false)
        .expect("create a second branch");
    let tag_object = native
        .tag(
            "v1",
            commit.as_object(),
            &commit.author(),
            "annotated",
            false,
        )
        .unwrap();
    let repo = Repository::open_exact(&path).unwrap();
    let id = ObjectId::parse_hex(ObjectFormat::Sha1, &oid.to_string()).unwrap();

    for name in ["HEAD", "main", "refs/heads/main", "topic", "main^{commit}"] {
        assert_eq!(repo.resolve(name).unwrap(), Some(id), "{name}");
    }
    let full = oid.to_string();
    for short in [&full[..7], &full[..12], full.as_str()] {
        assert_eq!(repo.resolve(short).unwrap(), Some(id), "{short}");
    }
    // A tag name resolves to the tag object, not to the commit it names;
    // peeling is spelled in the revision.
    let tag_id = ObjectId::parse_hex(ObjectFormat::Sha1, &tag_object.to_string()).unwrap();
    assert_ne!(tag_id, id);
    assert_eq!(repo.resolve("v1").unwrap(), Some(tag_id));
    assert_eq!(repo.resolve("v1^{commit}").unwrap(), Some(id));

    // A revision that names nothing is Ok(None), as `read_ref` reports it.
    for missing in [
        "refs/heads/absent",
        "absent",
        "0123456789abcdef0123456789abcdef01234567",
        "deadbee",
    ] {
        assert_eq!(repo.resolve(missing).unwrap(), None, "{missing}");
    }
    // A revision the native layer cannot parse is an error, not a miss.
    let error = repo.resolve("main@{").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ObjectRead);
    assert!(error.native().is_some());
}

#[test]
fn resolve_reports_an_unborn_head_as_missing_and_keeps_the_object_format() {
    let temp = TempDir::new().unwrap();
    let empty = temp.path().join("empty");
    init_repo(&empty, ObjectFormat::Sha1);
    let repo = Repository::open_exact(&empty).unwrap();
    assert_eq!(repo.resolve("HEAD").unwrap(), None);
    assert_eq!(repo.resolve("main").unwrap(), None);

    let sha256 = temp.path().join("sha256");
    let native = init_repo(&sha256, ObjectFormat::Sha256);
    let oid = seed_commit(&native);
    let repo = Repository::open_exact(&sha256).unwrap();
    let id = repo.resolve("main").unwrap().expect("main resolves");
    assert_eq!(id.format(), ObjectFormat::Sha256);
    assert_eq!(id.to_string(), oid.to_string());
    assert_eq!(repo.resolve(&oid.to_string()[..10]).unwrap(), Some(id));
}

#[test]
fn a_plain_directory_is_not_a_repository_and_other_open_failures_stay_open_errors() {
    let temp = TempDir::new().unwrap();
    let plain = temp.path().join("plain");
    fs::create_dir(&plain).unwrap();
    let missing = temp.path().join("missing");

    for path in [&plain, &missing] {
        for opened in [
            Repository::open_exact(path).err(),
            Repository::open_from_env(path).err(),
        ] {
            let error = opened.expect("a non-repository must not open");
            assert_eq!(error.kind(), ErrorKind::NotARepository, "{path:?}");
            let native = error.native().expect("native diagnostics are retained");
            // libgit2's GIT_ENOTFOUND, the raw code the native layer reports.
            assert_eq!(native.code, -3);
            assert!(!native.message.is_empty());
        }
    }

    // A repository whose configuration names an unknown extension is found,
    // and fails to open for another reason: that stays RepositoryOpen.
    let broken = temp.path().join("broken");
    init_repo(&broken, ObjectFormat::Sha1);
    let config = broken.join(".git").join("config");
    let mut text = fs::read_to_string(&config).unwrap();
    text = text.replace("repositoryformatversion = 0", "repositoryformatversion = 1");
    text.push_str("[extensions]\n\tgwzunknownextension = true\n");
    fs::write(&config, text).unwrap();
    for opened in [
        Repository::open_exact(&broken).err(),
        Repository::open_from_env(&broken).err(),
    ] {
        let error = opened.expect("an unsupported extension must not open");
        assert_eq!(error.kind(), ErrorKind::RepositoryOpen);
        assert!(error.native().is_some());
    }
}

#[test]
fn is_repository_answers_yes_no_and_leaves_real_failures_as_errors() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    init_repo(&repo, ObjectFormat::Sha1);
    let bare = temp.path().join("bare");
    git2::Repository::init_bare(&bare).unwrap();
    let plain = temp.path().join("plain");
    fs::create_dir(&plain).unwrap();
    let nested = repo.join("nested");
    fs::create_dir(&nested).unwrap();

    assert!(Repository::is_repository(&repo).unwrap());
    assert!(Repository::is_repository(&repo.join(".git")).unwrap());
    assert!(Repository::is_repository(&bare).unwrap());
    // No parent search: a directory inside a repository is not one.
    for no in [&plain, &nested, &temp.path().join("missing")] {
        assert!(!Repository::is_repository(no).unwrap(), "{no:?}");
    }
    // A repository that is found and cannot be opened is an error, not "no".
    let broken = temp.path().join("broken");
    init_repo(&broken, ObjectFormat::Sha1);
    let config = broken.join(".git").join("config");
    let mut text = fs::read_to_string(&config).unwrap();
    text = text.replace("repositoryformatversion = 0", "repositoryformatversion = 1");
    text.push_str("[extensions]\n\tgwzunknownextension = true\n");
    fs::write(&config, text).unwrap();
    let error = Repository::is_repository(&broken).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::RepositoryOpen);
}

#[test]
fn error_message_is_the_diagnostic_text_without_the_kind_prefix() {
    let temp = TempDir::new().unwrap();
    let native = Repository::open_exact(&temp.path().join("missing"))
        .err()
        .unwrap();
    let diagnostic = native.native().unwrap().message.clone();
    assert_eq!(native.message(), diagnostic);
    assert!(native.to_string().ends_with(&diagnostic));
    // A plain error has a message too.
    let plain = ObjectId::parse_hex(ObjectFormat::Sha1, "zz").unwrap_err();
    assert!(!plain.message().is_empty());
    assert!(plain.native().is_none());
}
