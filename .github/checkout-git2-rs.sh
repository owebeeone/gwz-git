#!/usr/bin/env bash
# Check out the git2-rs fork beside gwz-git at the commit .github/git2-rs.commit
# pins, with its vendored libgit2 submodule. gwz-git depends on it by path
# (`../git2-rs`, package `gwz-git2`), so every job that runs cargo on gwz-git
# needs it there. Same shape as gwz-core's script of the same name.
#
# Usage: checkout-git2-rs.sh [DIR]
# Clones into DIR/git2-rs. DIR defaults to the directory that holds the gwz-git
# checkout this script lives in. Refuses to touch an existing git2-rs, so a local
# run never clobbers a real checkout.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
crate="$(dirname "$here")"
parent="${1:-$(dirname "$crate")}"
dest="$parent/git2-rs"
commit="$(grep -m1 -E '^[0-9a-f]{40}$' "$here/git2-rs.commit")"
if [ -e "$dest" ]; then
  echo "checkout-git2-rs: $dest already exists; not touching it" >&2
  exit 1
fi
git init -q "$dest"
git -C "$dest" remote add origin https://github.com/owebeeone/git2-rs.git
git -C "$dest" fetch -q --depth 1 origin "$commit"
git -C "$dest" checkout -q --detach FETCH_HEAD
git -C "$dest" submodule update -q --init --depth 1
echo "checkout-git2-rs: git2-rs $commit at $dest"
