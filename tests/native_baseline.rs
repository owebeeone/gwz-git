use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Barrier};
use std::thread;

use gwz_git::{ErrorKind, ObjectFormat, ObjectId, Repository};
use tempfile::TempDir;

#[test]
fn native_runtime_is_qualified_vendored_libgit2() {
    let version = git2::Version::get();
    assert_eq!(version.libgit2_version(), (1, 9, 7));
    assert!(version.vendored());
}

fn init_repo(path: &Path, format: ObjectFormat) -> git2::Repository {
    let mut options = git2::RepositoryInitOptions::new();
    options.initial_head("main");
    let native_format = match format {
        ObjectFormat::Sha1 => git2::ObjectFormat::Sha1,
        ObjectFormat::Sha256 => git2::ObjectFormat::Sha256,
        _ => panic!("unsupported object format in fixture"),
    };
    options.object_format(native_format);
    git2::Repository::init_opts(path, &options).unwrap()
}

fn temp_dir(prefix: &str) -> TempDir {
    if cfg!(windows) {
        let root = Path::new("D:/gwz-tests");
        std::fs::create_dir_all(root).unwrap();
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(root)
            .unwrap()
    } else {
        TempDir::new().unwrap()
    }
}

fn tree(repo: &git2::Repository) -> git2::Oid {
    repo.treebuilder(None).unwrap().write().unwrap()
}

fn raw_commit(
    repo: &git2::Repository,
    tree: git2::Oid,
    parents: &[git2::Oid],
    author: &[u8],
    committer: &[u8],
    encoding: Option<&[u8]>,
    message: &[u8],
) -> git2::Oid {
    let mut data = Vec::new();
    data.extend_from_slice(b"tree ");
    data.extend_from_slice(tree.to_string().as_bytes());
    data.push(b'\n');
    for parent in parents {
        data.extend_from_slice(b"parent ");
        data.extend_from_slice(parent.to_string().as_bytes());
        data.push(b'\n');
    }
    data.extend_from_slice(b"author ");
    data.extend_from_slice(author);
    data.push(b'\n');
    data.extend_from_slice(b"committer ");
    data.extend_from_slice(committer);
    data.push(b'\n');
    if let Some(encoding) = encoding {
        data.extend_from_slice(b"encoding ");
        data.extend_from_slice(encoding);
        data.push(b'\n');
    }
    data.push(b'\n');
    data.extend_from_slice(message);
    repo.odb()
        .unwrap()
        .write(git2::ObjectType::Commit, &data)
        .unwrap()
}

fn seed_commit(repo: &git2::Repository) -> git2::Oid {
    let signature = git2::Signature::new(
        "Seed",
        "seed@example.invalid",
        &git2::Time::new(1_700_000_000, 330),
    )
    .unwrap();
    let tree = repo.find_tree(tree(repo)).unwrap();
    repo.commit(Some("HEAD"), &signature, &signature, "seed", &tree, &[])
        .unwrap()
}

fn clean_child(test_name: &str) -> bool {
    const MARKER: &str = "GWZ_GIT_G0_NO_GIT_CHILD";
    if env::var_os(MARKER).is_some() {
        return false;
    }
    let output = Command::new(env::current_exe().unwrap())
        .env_clear()
        .env("PATH", "")
        .env("GIT_DIR", "gwz-git-deliberately-not-a-repository")
        .env(MARKER, "1")
        .args(["--exact", test_name, "--nocapture", "--test-threads", "1"])
        .output()
        .unwrap();
    assert!(output.status.success(), "no-Git child failed");
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("test read_commit_runs_without_git_on_path ... ok"),
        "no-Git child did not execute the selected test: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    true
}

#[test]
fn read_commit_runs_without_git_on_path() {
    let name = "read_commit_runs_without_git_on_path";
    if clean_child(name) {
        return;
    }
    let temp = temp_dir("native-no-git");
    let path = temp.path().join("repo");
    let native = init_repo(&path, ObjectFormat::Sha1);
    let oid = seed_commit(&native);
    let repo = Repository::open_exact(&path).unwrap();
    assert_eq!(
        repo.read_commit(ObjectId::parse_hex(ObjectFormat::Sha1, &oid.to_string()).unwrap())
            .unwrap()
            .id
            .to_string(),
        oid.to_string()
    );
}

#[test]
fn raw_commit_preserves_bytes_parent_order_times_and_encoding() {
    let temp = temp_dir("native-raw");
    let path = temp.path().join("repo");
    let native = init_repo(&path, ObjectFormat::Sha1);
    let tree = tree(&native);
    let first = raw_commit(
        &native,
        tree,
        &[],
        b"A\xffuthor <a\xff@example.invalid> 1700000000 +0530",
        b"C <c@example.invalid> 1700000001 -0700",
        None,
        b"first",
    );
    let second = raw_commit(
        &native,
        tree,
        &[first],
        b"A <a@example.invalid> 1700000002 +0000",
        b"C <c@example.invalid> 1700000003 +0000",
        None,
        b"second",
    );
    let target = raw_commit(
        &native,
        tree,
        &[second, first],
        b"N\xffame <e\xff@example.invalid> 1700000004 +1245",
        b"C <c@example.invalid> 1700000005 -1145",
        Some(b"ISO-8859-1"),
        b"\nleading\n\xffmiddle\0tail\n",
    );

    let repo = Repository::open_exact(&path).unwrap();
    let record = repo
        .read_commit(ObjectId::parse_hex(ObjectFormat::Sha1, &target.to_string()).unwrap())
        .unwrap();
    assert_eq!(record.parents[0].to_string(), second.to_string());
    assert_eq!(record.parents[1].to_string(), first.to_string());
    assert_eq!(record.message, b"\nleading\n\xffmiddle\0tail\n");
    assert_eq!(record.encoding, Some(b"ISO-8859-1".to_vec()));
    assert_eq!(record.author.name, b"N\xffame");
    assert_eq!(record.author.email, b"e\xff@example.invalid");
    assert_eq!(record.author.seconds, 1_700_000_004);
    assert_eq!(record.author.offset_minutes, 765);
    assert_eq!(record.committer.seconds, 1_700_000_005);
    assert_eq!(record.committer.offset_minutes, -705);
}

#[test]
fn missing_references_are_returned_but_wrong_objects_and_malformed_commits_fail() {
    let temp = temp_dir("native-missing");
    let path = temp.path().join("repo");
    let native = init_repo(&path, ObjectFormat::Sha1);
    let tree = tree(&native);
    let missing_parent = git2::Oid::from_str("1111111111111111111111111111111111111111").unwrap();
    let missing_tree = git2::Oid::from_str("2222222222222222222222222222222222222222").unwrap();
    let target = raw_commit(
        &native,
        tree,
        &[missing_parent],
        b"A <a@example.invalid> 1 +0000",
        b"C <c@example.invalid> 2 +0000",
        None,
        b"missing parent",
    );
    let missing_tree_commit = raw_commit(
        &native,
        missing_tree,
        &[],
        b"A <a@example.invalid> 3 +0000",
        b"C <c@example.invalid> 4 +0000",
        None,
        b"missing tree",
    );
    let blob = native.blob(b"blob").unwrap();
    let signature = git2::Signature::now("Tagger", "tagger@example.invalid").unwrap();
    let target_object = native.find_object(target, None).unwrap();
    let tag = native
        .tag_annotation_create("tag-only", &target_object, &signature, "tag")
        .unwrap();
    let malformed = native
        .odb()
        .unwrap()
        .write(git2::ObjectType::Commit, b"not a commit\n")
        .unwrap();
    let repo = Repository::open_exact(&path).unwrap();
    for oid in [missing_parent, blob, tag, malformed] {
        let expected = native.find_commit(oid).err().unwrap();
        let error = repo
            .read_commit(ObjectId::parse_hex(ObjectFormat::Sha1, &oid.to_string()).unwrap())
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ObjectRead);
        let diagnostic = error.native().expect("native lookup failure retained");
        assert_eq!(diagnostic.code, expected.raw_code());
        assert_eq!(diagnostic.class, expected.raw_class() as i32);
        assert_eq!(diagnostic.message, expected.message());
    }

    let record = repo
        .read_commit(ObjectId::parse_hex(ObjectFormat::Sha1, &target.to_string()).unwrap())
        .unwrap();
    assert_eq!(
        record.parents,
        vec![ObjectId::parse_hex(ObjectFormat::Sha1, &missing_parent.to_string()).unwrap()]
    );
    let missing_tree_record = repo
        .read_commit(
            ObjectId::parse_hex(ObjectFormat::Sha1, &missing_tree_commit.to_string()).unwrap(),
        )
        .unwrap();
    assert_eq!(
        missing_tree_record.tree.to_string(),
        missing_tree.to_string()
    );
    assert_eq!(
        repo.read_commit(ObjectId::parse_hex(ObjectFormat::Sha1, &blob.to_string()).unwrap())
            .unwrap_err()
            .kind(),
        ErrorKind::ObjectRead
    );
    assert_eq!(
        repo.read_commit(ObjectId::parse_hex(ObjectFormat::Sha1, &tag.to_string()).unwrap())
            .unwrap_err()
            .kind(),
        ErrorKind::ObjectRead
    );
    assert_eq!(
        repo.read_commit(ObjectId::parse_hex(ObjectFormat::Sha1, &malformed.to_string()).unwrap())
            .unwrap_err()
            .kind(),
        ErrorKind::ObjectRead
    );
}

#[test]
fn sha256_reads_are_independent_and_concurrent() {
    let temp = temp_dir("native-sha256");
    let path = temp.path().join("repo");
    let native = init_repo(&path, ObjectFormat::Sha256);
    let tree = tree(&native);
    let mut expected = Vec::new();
    let mut parent = Vec::new();
    for index in 0..4 {
        let message = vec![b'a' + index as u8; 32 * 1024];
        let oid = raw_commit(
            &native,
            tree,
            &parent,
            b"A <a@example.invalid> 1700000000 +0000",
            b"C <c@example.invalid> 1700000000 +0000",
            None,
            &message,
        );
        expected.push((oid.to_string(), message));
        parent.clear();
        parent.push(oid);
    }
    let barrier = Arc::new(Barrier::new(expected.len() + 1));
    let workers = expected
        .iter()
        .map(|(expected, message)| {
            let path = path.clone();
            let expected = expected.clone();
            let message = message.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let repo = Repository::open_exact(&path).unwrap();
                let id = ObjectId::parse_hex(ObjectFormat::Sha256, &expected).unwrap();
                barrier.wait();
                for _ in 0..8 {
                    let record = repo.read_commit(id).unwrap();
                    assert_eq!(record.id.to_string(), expected);
                    assert_eq!(record.message, message);
                }
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    for worker in workers {
        worker.join().unwrap();
    }
}

#[test]
fn read_commit_leaves_refs_index_worktree_environment_and_cwd_unchanged() {
    let temp = temp_dir("native-readonly");
    let path = temp.path().join("repo");
    let native = init_repo(&path, ObjectFormat::Sha1);
    let oid = seed_commit(&native);
    let file = path.join("tracked.txt");
    fs::write(&file, b"worktree\n").unwrap();
    let mut index = native.index().unwrap();
    index.add_path(Path::new("tracked.txt")).unwrap();
    index.write().unwrap();
    let index_path = index.path().expect("index path").to_path_buf();
    let before_index = fs::read(&index_path).unwrap();
    let before_head = native.head().unwrap().target();
    let before_file = fs::read(&file).unwrap();
    let before_cwd = env::current_dir().unwrap();
    let before_environment = env::vars_os().collect::<std::collections::BTreeMap<_, _>>();
    drop(index);
    drop(native);

    let repository = Repository::open_exact(&path).unwrap();
    let record = repository
        .read_commit(ObjectId::parse_hex(ObjectFormat::Sha1, &oid.to_string()).unwrap())
        .unwrap();
    assert_eq!(record.id.to_string(), oid.to_string());
    assert_eq!(
        git2::Repository::open(&path)
            .unwrap()
            .head()
            .unwrap()
            .target(),
        before_head
    );
    assert_eq!(fs::read(&index_path).unwrap(), before_index);
    assert_eq!(fs::read(&file).unwrap(), before_file);
    assert_eq!(env::current_dir().unwrap(), before_cwd);
    assert_eq!(
        env::vars_os().collect::<std::collections::BTreeMap<_, _>>(),
        before_environment
    );
}

#[test]
fn exact_open_accepts_a_linked_worktree_root() {
    let temp = temp_dir("native-linked");
    let main = temp.path().join("main");
    let native = init_repo(&main, ObjectFormat::Sha1);
    let oid = seed_commit(&native);
    let linked = temp.path().join("linked");
    let status = Command::new("git")
        .args(["-C", main.to_str().unwrap(), "worktree", "add", "--detach"])
        .arg(&linked)
        .arg(oid.to_string())
        .status()
        .unwrap();
    assert!(status.success());
    let repo = Repository::open_exact(&linked).unwrap();
    assert!(repo.work_dir().is_some_and(|path| path.ends_with("linked")));
    assert_eq!(repo.object_format(), ObjectFormat::Sha1);
}

#[test]
fn stored_parents_ignore_shallow_and_explicit_graft_rewrites() {
    for format in [ObjectFormat::Sha1, ObjectFormat::Sha256] {
        for shallow in [true, false] {
            let temp = temp_dir("native-grafts");
            let path = temp.path().join("repo");
            let native = init_repo(&path, format);
            let tree = tree(&native);
            let first = seed_commit(&native);
            let second = raw_commit(
                &native,
                tree,
                &[],
                b"A <a@invalid> 2 +0000",
                b"C <c@invalid> 2 +0000",
                None,
                b"second",
            );
            let child = raw_commit(
                &native,
                tree,
                &[first, second],
                b"A <a@invalid> 3 +0000",
                b"C <c@invalid> 3 +0000",
                None,
                b"child",
            );
            let metadata = native
                .path()
                .join(if shallow { "shallow" } else { "info/grafts" });
            fs::create_dir_all(metadata.parent().unwrap()).unwrap();
            let contents = if shallow {
                format!("{child}\n")
            } else {
                format!("{child} {second} {first}\n")
            };
            drop(native);
            fs::write(&metadata, contents.as_bytes()).unwrap();
            let traversal = git2::Repository::open(&path).unwrap();
            let observed = traversal
                .find_commit(child)
                .unwrap()
                .parent_ids()
                .collect::<Vec<_>>();
            assert_eq!(observed, if shallow { vec![] } else { vec![second, first] });
            let record = Repository::open_exact(&path)
                .unwrap()
                .read_commit(ObjectId::parse_hex(format, &child.to_string()).unwrap())
                .unwrap();
            assert_eq!(record.tree.to_string(), tree.to_string());
            assert_eq!(
                record
                    .parents
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                vec![first.to_string(), second.to_string()]
            );
            assert_eq!(fs::read(&metadata).unwrap(), contents.as_bytes());
        }
    }
}

#[test]
fn malformed_grafts_preserve_the_native_error_class() {
    let temp = temp_dir("native-bad-grafts");
    let path = temp.path().join("repo");
    let native = init_repo(&path, ObjectFormat::Sha1);
    let metadata = native.path().join("info/grafts");
    fs::create_dir_all(metadata.parent().unwrap()).unwrap();
    drop(native);
    fs::write(&metadata, b"invalid\n").unwrap();
    let error = Repository::open_exact(&path)
        .err()
        .expect("malformed graft rejected");
    assert_eq!(error.kind(), ErrorKind::RepositoryOpen);
    let diagnostic = error.native().unwrap();
    // libgit2 1.9.7 include/git2/errors.h: GIT_ERROR_GRAFTS = 36.
    assert_eq!(diagnostic.class, 36);
    assert_eq!(diagnostic.code, -1);
    assert!(!diagnostic.message.is_empty());
    assert_eq!(fs::read(metadata).unwrap(), b"invalid\n");
}
