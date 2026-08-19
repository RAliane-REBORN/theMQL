# BUGS.md

Known bugs and unresolved issues. Empty when all known issues are resolved.
Update after every turn (see `MEMORY.md` standing rules).

## Format

One entry per bug. Use the template below. Close an entry with a `RESOLVED`
line + date + commit/PR reference when fixed.

```
### BUG-NNNN: <short title>

- Discovered: YYYY-MM-DD
- Severity: blocker | major | minor | cosmetic
- Subsystem: themql-<crate> | specs | prompts | docs | ci
- Status: open | in-progress | resolved
- Symptom: what is observed
- Expected: what the spec or correct behaviour requires
- Reproduction: minimal steps / command
- Root cause: (filled when known)
- Fix: (filled when known; include commit hash)
- Follow-up: regression test, spec clarification, etc.
```

## Open bugs

(none)

## Resolved bugs

(none)

## Known limitations (not bugs, but tracked alongside)

These are spec-acknowledged gaps, not bugs:

- `[workspace.dependencies]` versions are resolved against crates.io
  (2026-08-19), but `valkey` is alpha (`0.0.0-alpha5`) and `cachelito` is a
  proc-macro for function caching — both may need reassessment for the L1/L3
  use cases in Phase 1.
- No real Rust source beyond the two 1-line binary `fn main(){}` stubs and
  0-line `lib.rs` stubs. Phase 1 starts populating `src/`.
- No CI guard script yet. The architectural invariant "crate names in
  specs match workspace members" is currently enforced only by manual
  review.
- No `opencode.json`. The OpenCode-specific config file is not yet written;
  `AGENTS.md` exists as the agent convention file.
- `mold`, `sccache`, `cargo-deny`, `cargo-machete`, `cargo-bloat` not
  installed in this environment. Config files (`deny.toml`,
  `.cargo/config.toml`, `TETANUS.md`) are ready for when they are.
  Install: `apt install mold`, `cargo install sccache cargo-deny
  cargo-machete cargo-bloat`. Then uncomment the relevant lines in
  `.cargo/config.toml`.
- No fuzzing harness, no dependency-audit pipeline, no secret-management
  policy. Tracked in `SECURITY.md` known limitations.