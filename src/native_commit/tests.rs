//! Tests of the native commit and tag route.
//!
//! * `route` proves each condition of the route rule picks the Git CLI, by
//!   the route chosen and, for hooks, by the hook's side effect;
//! * `differential` runs the same operation natively on one repository and
//!   through the Git CLI on an identical twin, and compares object ids,
//!   references, reflogs, the index and failure codes;
//! * `route_config` covers the route's inputs beyond hooks and signing;
//! * `environment` does the same with Git's own variables set, in a clean
//!   child process.

mod compat;
#[macro_use]
mod support;

// Its tests are Unix-only (shell hooks and a mock signer), and so are its helpers.
cfg_if::cfg_if! {
    if #[cfg(unix)] {
        mod commit_tag_characterization;
    }
}
mod commit_tag_identity_characterization;
mod differential_commit;
mod differential_edges;
mod differential_tag;
mod environment;
mod notes;
mod route_commit;
mod route_config;
mod route_tag;
