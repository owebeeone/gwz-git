# gwz-git

Follow the workspace AGENTS.md and AGENTS_GWZ.md. Use tests first.

The accepted G0 contract is in the sibling core's dev-docs/GwzGitLibraryDesign.md
and GwzGitLibraryApi.md. This crate owns single-repository behavior only; do not
introduce workspace, protocol, endpoint or product Git subprocess dependencies.
The one subprocess is the Git CLI route of native commit and tag (moved here by step 5.2,
operator decisions D-G and D-K), launched through the caller's `ProcessGate` and listed in
`scripts/process_globals_allowlist.json`; any other spawn needs a new decision.

Keep native types private. All control-flow bodies use braces; Rust conditional
sections require cfg_if blocks or enclosing platform modules. Verify disabled
branches as source too. Do not change dependencies or scope without the integrator.
