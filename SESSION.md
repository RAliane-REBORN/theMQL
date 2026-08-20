# SESSION.md

Current session state. Update at the end of every turn (see `MEMORY.md`
standing rules). When a session ends, fold the in-flight items into
`HANDOVER.md`.

## Current session

- Date: 2026-08-20
- Mode: build
- Agent: opencode (glm-5.2:cloud)
- Branch: `feat/phase-1-spec-deepening` (stacked on
  `feat/v0.1-spec-and-workspace-skeleton`, PR #1 open)

## Just-completed turn

Phase 1 complete. Three stages:

1. **nalgebra amendment** — nalgebra 0.35 replaces ndarray for non-ML
   numerics (EKF, covariance, quaternion, rotation, linear algebra).
   ndarray demoted to raw ML-side buffers only. Updated SYSTEM.md,
   ARCHITECT.md, SPEC.toml, Cargo.toml, AGENTS.md, MEMORY.md.
2. **Stage 1 — spec deepening** — deepened ALL 19 specs from thin/medium
   to deep with concrete types/traits/signatures. User decisions applied:
   Selection = filter + projection combined (core.toml); TWO separate
   runtime traits DesktopRuntime + EmbeddedRuntime, no shared trait
   (runtime.toml); EstimatorState = 21-dim (state_estimation.toml).
3. **Stage 2 — all 19 crates implemented** — each crate now has real Rust
   types/traits + unit tests. See CHANGELOG.md 2026-08-20 for the per-crate
   breakdown.

Files touched this turn: prompts/SYSTEM.md, prompts/ARCHITECT.md,
SPEC.toml, Cargo.toml, AGENTS.md, all 19 specs/*.toml, all 19
crates/themql-*/{Cargo.toml,src/*}, + 8 living docs.

## State of the repository

- v0.1 spec + workspace skeleton + toolchain hardening + Phase 1 complete.
- ALL 19 crates now have real `src/` content (traits + types + error types +
  unit tests). No 0-line stubs remain.
- 183 unit tests + 1 doc test = 184 tests, all pass.
- Full validation green:
  - `cargo fmt --all --check` — clean.
  - `cargo check --workspace` — passes for all 19 crates.
  - `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings.
  - `cargo test --workspace` — 184 tests pass.
  - `python3 tomllib` — all TOML files parse.
  - `cargo metadata --no-deps` — resolves.
- The four safety-critical crates (gnc, estimation, inference, artifact)
  comply with TETANUS.md (forbid(unsafe_code), deny(warnings),
  clippy::pedantic, no recursion, fixed loop bounds, no heap alloc after
  init where required, functions <= 60 lines, >= 2 assertions per function,
  no unwrap()/expect() in non-test code). nalgebra 0.35 used for all non-ML
  numerics. `tch` deliberately not added to inference (libtorch build dep);
  the InferenceEngine trait is defined without it.
- New workspace deps added: nalgebra 0.35, uuid 1 (v7+serde), blake3 1,
  thiserror 2, serde-big-array 0.5.
- Working tree has uncommitted changes. Not yet committed.

## In-flight work

None. Phase 1 is complete.

## Next plausible actions (suggestions, not commitments)

1. Commit the work (user has not requested commits yet).
2. Implement full nonlinear quaternion EKF dynamics + Jacobian-based
   Kalman gain in themql-estimation (v0.1 uses simplified identity-gain
   updates as a placeholder).
3. Wire a real SHA-256 (or BLAKE3) into themql-artifact's HashValidator
   (v0.1 uses a placeholder fold hash — must replace before deployment).
4. Add a real `tch`-backed `InferenceEngine` impl in themql-inference once
   libtorch is available in the build env.
5. Reconcile CacheKey (defined locally in themql-cache) with themql-query's
   CacheKeyer — do not duplicate the key type silently.
6. Wire heavy deps (tch, polars, dioxus, ratatui, embassy) into the
   training/analysis/desktop/embedded crates behind the trait surfaces.
7. Install cargo-deny, cargo-machete, cargo-bloat, sccache, mold; run the
   safety-critical validation gate.
8. Add a CI guard script (TOML parse + crate-name-vs-workspace invariant).
9. Write `opencode.json`.

## Open questions / blockers

None. Awaiting user direction.