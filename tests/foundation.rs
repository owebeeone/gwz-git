use std::fs;
use std::path::Path;
use std::thread;

use gwz_git::{ErrorKind, ObjectFormat, ObjectId, Repository};
use tempfile::TempDir;

fn init_repo(path: &Path) {
    let mut options = git2::RepositoryInitOptions::new();
    options.initial_head("main");
    git2::Repository::init_opts(path, &options).unwrap();
}

fn temp_dir(prefix: &str) -> TempDir {
    if cfg!(windows) {
        let root = Path::new("D:/gwz-tests");
        fs::create_dir_all(root).unwrap();
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(root)
            .unwrap()
    } else {
        TempDir::new().unwrap()
    }
}

#[test]
fn object_ids_are_full_format_specific_and_round_trip() {
    for (format, width) in [(ObjectFormat::Sha1, 40), (ObjectFormat::Sha256, 64)] {
        for bad in [
            "".to_owned(),
            "a".repeat(width - 1),
            "a".repeat(width + 1),
            format!("{}g", "a".repeat(width - 1)),
            format!(" {}", "a".repeat(width - 1)),
            "é".repeat(width / 2),
        ] {
            let error = ObjectId::parse_hex(format, &bad).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::InvalidObjectId);
            assert!(error.native().is_none());
        }
        let id = ObjectId::parse_hex(format, &"Ab".repeat(width / 2)).unwrap();
        assert_eq!(ObjectId::parse_hex(format, &id.to_string()).unwrap(), id);
    }
    assert_ne!(
        ObjectId::parse_hex(ObjectFormat::Sha1, &"0".repeat(40)).unwrap(),
        ObjectId::parse_hex(ObjectFormat::Sha256, &"0".repeat(64)).unwrap(),
    );
    let sha1 = ObjectId::parse_hex(
        ObjectFormat::Sha1,
        "ABCDEF0123456789ABCDEF0123456789ABCDEF01",
    )
    .unwrap();
    assert_eq!(sha1.format(), ObjectFormat::Sha1);
    assert_eq!(sha1.as_bytes().len(), 20);
    assert_eq!(sha1.to_string(), "abcdef0123456789abcdef0123456789abcdef01");
    assert_eq!(
        sha1,
        ObjectId::parse_hex(ObjectFormat::Sha1, &sha1.to_string()).unwrap()
    );
    assert_eq!(
        sha1,
        ObjectId::parse_hex(ObjectFormat::Sha1, &sha1.to_string().to_uppercase()).unwrap()
    );

    let sha256 = ObjectId::parse_hex(
        ObjectFormat::Sha256,
        "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
    )
    .unwrap();
    assert_eq!(sha256.format(), ObjectFormat::Sha256);
    assert_eq!(sha256.as_bytes().len(), 32);
    assert_eq!(
        sha256.to_string(),
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
    );

    assert_eq!(
        ObjectId::parse_hex(ObjectFormat::Sha1, &sha256.to_string())
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidObjectId
    );
    assert_eq!(
        ObjectId::parse_hex(ObjectFormat::Sha1, "0123")
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidObjectId
    );
    assert!(
        ObjectId::parse_hex(
            ObjectFormat::Sha1,
            "0123456789abcdef0123456789abcdef0123456g"
        )
        .is_err()
    );
}

#[test]
fn exact_open_handles_normal_bare_unborn_and_rejects_parent_discovery() {
    let temp = temp_dir("foundation-open");
    let normal = temp.path().join("normal");
    init_repo(&normal);
    let repo = Repository::open_exact(&normal).unwrap();
    assert!(repo.work_dir().is_some_and(|path| path.ends_with("normal")));
    assert!(repo.git_dir().join("config").is_file());
    assert_eq!(repo.git_dir(), repo.common_dir());
    assert_eq!(repo.object_format(), ObjectFormat::Sha1);
    let via_git_dir = Repository::open_exact(&normal.join(".git")).unwrap();
    assert_eq!(via_git_dir.git_dir(), repo.git_dir());

    let bare = temp.path().join("bare");
    git2::Repository::init_bare(&bare).unwrap();
    let bare_repo = Repository::open_exact(&bare).unwrap();
    assert_eq!(bare_repo.work_dir(), None);
    assert!(bare_repo.git_dir().ends_with("bare"));
    assert!(bare_repo.common_dir().ends_with("bare"));

    let nested = normal.join("nested");
    fs::create_dir(&nested).unwrap();
    let error = match Repository::open_exact(&nested) {
        Ok(_) => panic!("nested path unexpectedly discovered a repository"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), ErrorKind::RepositoryOpen);
    assert!(Repository::open_exact(&temp.path().join("missing")).is_err());
    let mismatch = Repository::open_exact(&normal)
        .unwrap()
        .read_commit(ObjectId::parse_hex(ObjectFormat::Sha256, &"0".repeat(64)).unwrap())
        .unwrap_err();
    assert_eq!(mismatch.kind(), ErrorKind::ObjectFormatMismatch);
    assert!(mismatch.native().is_none());
}

#[test]
fn repository_ownership_moves_between_workers_and_results_are_independent() {
    fn assert_send<T: Send>() {}
    assert_send::<Repository>();
    let temp = temp_dir("foundation-send");
    let path = temp.path().join("repo");
    let native = {
        init_repo(&path);
        let native = git2::Repository::open(&path).unwrap();
        let signature = git2::Signature::now("Send", "send@example.invalid").unwrap();
        let oid = {
            let tree = native.treebuilder(None).unwrap().write().unwrap();
            let tree = native.find_tree(tree).unwrap();
            native
                .commit(None, &signature, &signature, "moved", &tree, &[])
                .unwrap()
        };
        (native, oid)
    };
    let id = ObjectId::parse_hex(ObjectFormat::Sha1, &native.1.to_string()).unwrap();
    drop(native.0);
    let repository = Repository::open_exact(&path).unwrap();
    let worker = thread::spawn(move || {
        assert_eq!(repository.object_format(), ObjectFormat::Sha1);
        let record = repository.read_commit(id).unwrap();
        drop(repository);
        record
    });
    let record = worker.join().unwrap();
    assert_eq!(record.id, id);
    assert_eq!(record.message, b"moved");
}

#[test]
fn native_open_error_owns_diagnostics_after_native_handle_lifetime_ends() {
    let temp = temp_dir("foundation-error");
    let error = match Repository::open_exact(&temp.path().join("missing")) {
        Ok(_) => panic!("missing path unexpectedly opened"),
        Err(error) => error,
    };
    let kind = error.kind();
    let diagnostic = error
        .native()
        .map(|native| (native.code, native.class, native.message.clone()));
    drop(error);
    assert_eq!(kind, ErrorKind::RepositoryOpen);
    let (code, class, message) = diagnostic.expect("native open error diagnostics");
    assert_ne!(code, 0);
    assert_ne!(class, 0);
    assert!(!message.is_empty());
}
