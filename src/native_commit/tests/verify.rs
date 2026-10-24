//! The AD1 self-verification of tags (round 1 remediation, S-P2-1 and S-P3-2):
//! existence is an exact reference lookup, never a revision lookup, so a tag
//! name that looks like `git describe` output or a branch that shares a tag's
//! name cannot stand in for the tag; and a verification read that fails after
//! the write is reported as an `Operation` error that says the write happened.

use std::fs;

use super::support::*;
use crate::error::ErrorKind;
use crate::native_commit::verify;

/// A tag name in `git describe` form for the commit at HEAD: `t-1-g<abbrev>`.
fn describe_form(path: &std::path::Path) -> String {
    format!("t-1-g{}", git(path, &["rev-parse", "--short=7", "HEAD"]))
}

#[test]
fn a_tag_named_like_describe_output_is_created_and_deleted_on_both_routes() {
    hermetic!();
    let twin = twin(seeded);
    let name = describe_form(&twin.native);
    assert!(matches!(twin.tag_create(&name, None), Outcome::Done(_)));
    assert!(matches!(twin.tag_delete(&name), Outcome::Done(_)));
    for path in [&twin.native, &twin.cli] {
        assert_eq!(git(path, &["tag", "-l"]), "", "no tag is left in {path:?}");
    }
}

#[test]
fn an_annotated_describe_form_tag_also_round_trips_on_both_routes() {
    hermetic!();
    let twin = twin(seeded);
    let name = describe_form(&twin.native);
    assert!(matches!(
        twin.tag_create(&name, Some("note")),
        Outcome::Done(_)
    ));
    assert!(matches!(twin.tag_delete(&name), Outcome::Done(_)));
    assert_eq!(git(&twin.native, &["tag", "-l"]), "");
}

#[test]
fn tag_created_refuses_a_describe_form_name_that_is_not_a_tag() {
    hermetic!();
    let lab = lab();
    let name = describe_form(&lab.path);
    let error = verify::tag_created(&lab.path, &name).unwrap_err();
    assert!(
        error.message().contains("missing after creation"),
        "{error:?}"
    );
}

#[test]
fn tag_deleted_is_not_fooled_by_a_branch_named_like_the_tag_ref() {
    hermetic!();
    let twin = twin(seeded);
    // `refs/heads/refs/tags/X` is what the revision rules reach for "refs/tags/X" once the tag is gone.
    twin.both(|path| {
        git(path, &["update-ref", "refs/heads/refs/tags/X", "HEAD"]);
    });
    assert!(matches!(twin.tag_create("X", None), Outcome::Done(_)));
    assert!(matches!(twin.tag_delete("X"), Outcome::Done(_)));
    assert_eq!(git(&twin.native, &["tag", "-l"]), "");
    verify::tag_deleted(&twin.native, "X").unwrap();
}

#[test]
fn a_missing_tag_is_missing_and_a_present_one_resolves_to_its_commit() {
    hermetic!();
    let lab = lab();
    lab.backend
        .tag_create(&lab.path, "v1", Some("note"), false)
        .unwrap();
    let outcome = verify::tag_created(&lab.path, "v1").unwrap();
    assert_eq!(
        outcome.commit.to_string(),
        git(&lab.path, &["rev-parse", "HEAD"])
    );
    let error = verify::tag_created(&lab.path, "v2").unwrap_err();
    assert!(
        error.message().contains("missing after creation"),
        "{error:?}"
    );
    let error = verify::tag_deleted(&lab.path, "v1").unwrap_err();
    assert!(
        error.message().contains("still present after delete"),
        "{error:?}"
    );
    verify::tag_deleted(&lab.path, "v2").unwrap();
}

#[test]
fn a_verification_read_that_fails_after_the_write_says_so_with_the_operation_kind() {
    hermetic!();
    let lab = lab();
    // A loose reference with garbage in it: the write happened, the read back cannot be trusted.
    fs::create_dir_all(lab.path.join(".git/refs/tags")).unwrap();
    fs::write(lab.path.join(".git/refs/tags/bad"), b"not a reference\n").unwrap();
    let gone = lab.path.join("not-a-repository");
    fs::create_dir(&gone).unwrap();
    for error in [
        verify::tag_created(&lab.path, "bad").unwrap_err(),
        verify::tag_deleted(&lab.path, "bad").unwrap_err(),
        verify::tag_created(&gone, "bad").unwrap_err(),
        verify::tag_deleted(&gone, "bad").unwrap_err(),
        verify::head_after(&gone, "commit").unwrap_err(),
    ] {
        assert_eq!(error.kind(), ErrorKind::Operation, "{error:?}");
        assert!(
            error
                .message()
                .contains("written; verification read failed: "),
            "{error:?}"
        );
    }
    // Before the write, the same read keeps its own kind.
    assert_eq!(
        verify::head(&gone).unwrap_err().kind(),
        ErrorKind::NotARepository
    );
}
