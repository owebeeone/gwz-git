//! Step 5.5: the index snapshot that gwz-core's `Git2Backend::repository_index`
//! used to build from the native index itself, now `Repository::index_snapshot`.

use std::fs;
use std::path::Path;

use gwz_git::{ObjectFormat, ObjectId, Repository};
use tempfile::TempDir;

fn init(path: &Path, format: git2::ObjectFormat) -> git2::Repository {
    let mut options = git2::RepositoryInitOptions::new();
    options.initial_head("main");
    options.object_format(format);
    git2::Repository::init_opts(path, &options).unwrap()
}

fn staged(native: &git2::Repository, relative: &str, content: &str) {
    fs::write(native.workdir().unwrap().join(relative), content).unwrap();
    let mut index = native.index().unwrap();
    index.add_path(Path::new(relative)).unwrap();
    index.write().unwrap();
}

#[test]
fn an_index_snapshot_carries_every_entry_the_native_index_holds() {
    let temp = TempDir::new().unwrap();
    let native = init(temp.path(), git2::ObjectFormat::Sha1);
    staged(&native, "a.txt", "alpha\n");
    staged(&native, "b.txt", "bravo bravo\n");

    let snapshot = Repository::open_exact(temp.path())
        .unwrap()
        .index_snapshot()
        .unwrap();

    let index = native.index().unwrap();
    assert_eq!(snapshot.path.as_deref(), index.path());
    assert!(snapshot.path.as_ref().unwrap().ends_with("index"));
    assert_eq!(snapshot.entries.len(), 2);
    for (entry, expected) in snapshot.entries.iter().zip(index.iter()) {
        assert_eq!(entry.path, expected.path);
        assert_eq!(entry.id.as_bytes(), expected.id.as_bytes());
        assert_eq!(entry.id.format(), ObjectFormat::Sha1);
        assert_eq!(entry.mode, expected.mode);
        assert_eq!(entry.flags, expected.flags);
        assert_eq!(entry.flags_extended, expected.flags_extended);
        assert_eq!(
            entry.ctime,
            (expected.ctime.seconds(), expected.ctime.nanoseconds())
        );
        assert_eq!(
            entry.mtime,
            (expected.mtime.seconds(), expected.mtime.nanoseconds())
        );
        assert_eq!(
            entry.stat,
            [
                expected.dev,
                expected.ino,
                expected.uid,
                expected.gid,
                expected.file_size
            ]
        );
    }
    assert_eq!(snapshot.entries[0].path, b"a.txt");
    assert_eq!(snapshot.entries[0].stat[4], 6, "file size of a.txt");
    assert_eq!(snapshot.entries[1].stat[4], 12, "file size of b.txt");
    assert_eq!(
        snapshot.entries[0].id,
        ObjectId::parse_hex(
            ObjectFormat::Sha1,
            &git2::Oid::hash_object(git2::ObjectType::Blob, b"alpha\n")
                .unwrap()
                .to_string()
        )
        .unwrap()
    );
}

#[test]
fn an_empty_index_has_no_entries_and_a_path() {
    let temp = TempDir::new().unwrap();
    init(temp.path(), git2::ObjectFormat::Sha1);

    let snapshot = Repository::open_exact(temp.path())
        .unwrap()
        .index_snapshot()
        .unwrap();

    assert!(snapshot.entries.is_empty());
    assert!(snapshot.path.unwrap().ends_with("index"));
}

#[test]
fn entries_keep_mode_stage_and_extended_flags_as_stored() {
    let temp = TempDir::new().unwrap();
    let native = init(temp.path(), git2::ObjectFormat::Sha1);
    let mut index = native.index().unwrap();
    let entry = |path: &str, mode: u32, flags: u16, flags_extended: u16| git2::IndexEntry {
        ctime: git2::IndexTime::new(0, 0),
        mtime: git2::IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode,
        uid: 0,
        gid: 0,
        file_size: 0,
        id: git2::Oid::ZERO_SHA1,
        flags,
        flags_extended,
        path: path.as_bytes().to_vec(),
    };
    index
        .add_frombuffer(&entry("run.sh", 0o100755, 0, 0), b"#!/bin/sh\n")
        .unwrap();
    index
        .add_frombuffer(&entry("plain.txt", 0o100644, 0, 0), b"plain\n")
        .unwrap();
    index.write().unwrap();

    let snapshot = Repository::open_exact(temp.path())
        .unwrap()
        .index_snapshot()
        .unwrap();

    let by_path = |path: &[u8]| {
        snapshot
            .entries
            .iter()
            .find(|entry| entry.path == path)
            .unwrap()
    };
    assert_eq!(by_path(b"run.sh").mode, 0o100755);
    assert_eq!(by_path(b"plain.txt").mode, 0o100644);
    assert_eq!(
        snapshot.entries[0].path, b"plain.txt",
        "index order is by path"
    );
}

#[test]
fn a_sha256_repository_yields_32_byte_ids() {
    let temp = TempDir::new().unwrap();
    let native = init(temp.path(), git2::ObjectFormat::Sha256);
    staged(&native, "a.txt", "alpha\n");

    let snapshot = Repository::open_exact(temp.path())
        .unwrap()
        .index_snapshot()
        .unwrap();

    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries[0].id.format(), ObjectFormat::Sha256);
    assert_eq!(snapshot.entries[0].id.as_bytes().len(), 32);
}

#[test]
fn a_bare_repository_reports_an_empty_index_as_libgit2_does() {
    let temp = TempDir::new().unwrap();
    let native = git2::Repository::init_bare(temp.path()).unwrap();
    let expected = native.index().unwrap();

    let snapshot = Repository::open_exact(temp.path())
        .unwrap()
        .index_snapshot()
        .unwrap();

    assert!(snapshot.entries.is_empty());
    assert_eq!(snapshot.path.as_deref(), expected.path());
}

#[test]
fn the_snapshot_owns_its_data_and_is_not_a_view_of_later_changes() {
    let temp = TempDir::new().unwrap();
    let native = init(temp.path(), git2::ObjectFormat::Sha1);
    staged(&native, "a.txt", "alpha\n");
    let repo = Repository::open_exact(temp.path()).unwrap();
    let before = repo.index_snapshot().unwrap();
    drop(repo);

    staged(&native, "b.txt", "bravo\n");
    let after = Repository::open_exact(temp.path())
        .unwrap()
        .index_snapshot()
        .unwrap();

    assert_eq!(before.entries.len(), 1);
    assert_eq!(after.entries.len(), 2);
    assert_eq!(before.entries[0], after.entries[0]);
}
