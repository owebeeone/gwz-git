# gwz-git

Follow the workspace AGENTS.md and AGENTS_GWZ.md. Use tests first.

The accepted G0 contract is in the sibling core's dev-docs/GwzGitLibraryDesign.md
and GwzGitLibraryApi.md. This crate owns single-repository behavior only; do not
introduce workspace, protocol, endpoint or product Git subprocess dependencies.

Keep native types private. All control-flow bodies use braces; Rust conditional
sections require cfg_if blocks or enclosing platform modules. Verify disabled
branches as source too. Do not change dependencies or scope without the integrator.
