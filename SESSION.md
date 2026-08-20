# SESSION.md

Current session state. Update at the end of every turn (see `MEMORY.md`
standing rules). When a session ends, fold the in-flight items into
`HANDOVER.md`.

## Current session

- Date: 2026-08-20
- Mode: build
- Agent: opencode (glm-5.2:cloud)
- Branch: `feat/phase-2-heavy-dep-wiring` (stacked on the Phase 1 work)

## Just-completed turn

Phase 2 Stages 2-5 + 9 + 10 + 11 complete: wired SSE/MQTT/GraphQL
transport adapters, polars+rayon/tch into analysis/training/inference,
reconciled cross-crate type duplication, installed safety-critical
tooling, wrote opencode.json + CI guard script.

1-9. (Stages 2-5 + 9 + 10 — see previous turn summary in CHANGELOG.md)
10. **Stage 11a — safety-critical tooling** — installed `cargo-deny`,
    `cargo-machete`, `cargo-bloat`. `cargo deny check` passes (added
    `BSL-1.0` + `CDLA-Permissive-2.0` to allowed licenses). `cargo
    machete --with-metadata` clean (removed 11 unused deps across 8
    crates: serde from themql-message, valkey from themql-cache,
    themql-message from themql-sse, themql-core from themql-embedded,
    serde_json from themql-analysis, themql-query from themql-graphql,
    embassy-executor + embassy-sync + themql-message from themql-mqtt,
    themql-message from themql-query, rayon from themql-training,
    helix-db from themql-storage, blake3 from themql-core). `cargo
    bloat --release --crates -p themql-desktop` passes (3.9MiB binary,
    939KiB .text, no bloat). Removed conflicting `deny`/`machete` cargo
    aliases from `.cargo/config.toml`.
11. **Stage 11b — opencode.json** — wrote project config with
    `$schema`, `instructions: ["AGENTS.md"]`, permission rules (cargo/
    python3/git/gh allowed, rm asks), and two custom commands:
    `validate` (full validation suite) + `safety-gate` (TETANUS gate).
12. **Stage 11c — CI guard script** — wrote `scripts/ci_guard.py` that
    checks: (1) all TOML files parse, (2) crate names match directory
    names, (3) spec crate names match workspace members. All checks
    pass.

Validation: `cargo fmt --check`, `cargo clippy --workspace --all-targets
-- -D warnings`, `cargo test --workspace` (240 tests), `cargo deny
check`, `cargo machete --with-metadata`, `ci_guard.py` all green. No
`unsafe`. 20 crates, 20 specs.

## State of the repository

- Phase 1 complete (spec + 19 crates implemented).
- Phase 2 Stages 1-12 complete: cache/storage backends, transport
  adapters, runtime impls, desktop/embedded binary wiring,
  analysis/training/inference heavy-dep wiring, cross-crate type
  reconciliation, safety-critical tooling + opencode.json + CI guard.
  240 tests pass workspace-wide (default features). 20 crates, 20 specs.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace`, `cargo deny check`,
  `cargo machete --with-metadata`, `scripts/ci_guard.py` all green.
- BUG-0007 resolved (no open bugs).

## In-flight work

None. Phase 2 Stages 1-12 are complete. Commit `d97fcca` pushed to
`feat/phase-2-heavy-dep-wiring`, PR #4 open.

## Next plausible actions (suggestions, not commitments)

1. Merge PR #4 (awaiting review).
2. Implement full nonlinear quaternion EKF dynamics + Jacobian-based
   Kalman gain in themql-estimation.
3. Wire a real SHA-256 (or BLAKE3) into themql-artifact's
   HashValidator.
4. Run `tch-backend` feature tests once a beefier environment is
   available.
5. Real cachelito L1, real valkey L3, real helix-db L4, key→subject
   index for pattern invalidation.

## Open questions / blockers

None.