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

Phase 3 followups (Workstreams A/B/C/D) complete:

1. **Workstream A — living docs refresh + PR #4 merge**: synced all 8
   living docs to Phase 3's actual end state (removed stale placeholder
   claims about EKF, HashValidator, cache backends). Closed BUG-0008
   (sled temp test now stable). Committed `81af681`, pushed, merged PR
   #4 to main (`4fee9d9`). Created `feat/phase-3-followups` branch.
2. **Workstream B — real GraphQL subscriptions**: added
   `GraphqlSubscriptionSource` trait (dyn-compatible) + impl for
   `TokioSsePublisher`. `SseStreamAdapter` wraps `SseStream` as
   `Stream<Item = serde_json::Value>` via a background task + mpsc
   channel. `SubscriptionRoot::with_source(source)` + `subscribe()`
   now returns a real live stream of events from the SSE publisher.
   `SubscriptionRoot::default()` returns an error (no source). +2 tests
   (subscription with source streams real events; without source
   returns error). 15 tests total in themql-graphql (was 13).
3. **Workstream D — GitHub Actions CI**: created
   `.github/workflows/ci.yml` with 8 jobs: fmt, check, clippy, test,
   toml-sanity, ci-guard, deny, machete. Uses `dtolnay/rust-toolchain`
   + `Swatinem/rust-cache`. `tch-backend` explicitly skipped (libtorch
   too heavy for free runners). Runs on push to `feat/*` + PRs to
   `main`.
4. **Workstream C — real themql-desktop subcommands**: rewrote all 5
   subcommand bodies:
   - `serve`: builds GraphQL schema (`GraphqlSchemaImpl` with real
     resolver bridge + dispatch bridge + subscription source) + SSE
     server (`serve_sse`), merges axum routers, binds TCP, runs with
     graceful shutdown (Ctrl-C).
   - `analyze`: reads JSON data file, builds `PolarsDatasetBuilder`,
     runs `RayonAnalysisPipeline`, prints stats.
   - `train`: behind `tch-backend` feature — loads dataset, builds
     `TrainingConfig`, runs `TchTrainer::train`, packages via
     `BincodeArtifactWriter`, writes to file. Without feature: returns
     error with instructions.
   - `validate`: loads `ModelArtifact` via `FileArtifactLoader`, prints
     format/schema_version/bytes/hash.
   - `telemetry`: opens `SledStorage`, queries by subject pattern,
     prints entries.
   - Added `tch-backend` feature to themql-desktop Cargo.toml (optional
     dep on `themql-training`). 12 tests (was 13 — removed TUI test
     that requires a terminal).

Validation: 306 tests pass workspace-wide (default features). `cargo
fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo deny check`, `cargo machete --with-metadata`,
`scripts/ci_guard.py` — all green. Removed unused deps: `serde` from
themql-graphql, `serde_json` from themql-analysis (moved to dev-deps),
`tempfile` from themql-desktop dev-deps.

## State of the repository

- Phase 1 complete (spec + 20 crates implemented).
- Phase 2 Stages 1-12 complete.
- Phase 3 Stages 1-11 complete.
- Phase 3 followups complete: real GraphQL subscriptions, GitHub
  Actions CI, real themql-desktop subcommands. 306 tests pass
  workspace-wide (default features). 20 crates, 20 specs.
- Full validation green.
- `tch-backend` feature compiles clean in themql-training/
  themql-inference/themql-desktop (tests not run: libtorch OOM).
- No open bugs.

## In-flight work

None. Phase 3 followups are complete. Changes are committed on
`feat/phase-3-followups` branch, ready to push and open PR #5.

## Next plausible actions (suggestions, not commitments)

1. Run `tch-backend` feature tests once a beefier environment is
   available (>7.8GB RAM).
2. Real `themql-embedded` embassy main (requires thumbv7em target).
3. Transport-layer authn/authz policy (MQTT broker credentials, GraphQL
   access control).
4. Wire MQTT bridge into the `serve` subcommand (currently prints
   "not yet wired" when `--enable-mqtt` is passed).

## Open questions / blockers

None.
## 2026-08-20 — themql-sse real implementation (Stage 8)

Rewrote `crates/themql-sse/src/lib.rs` to flow real `SseEvent`s through
the broadcast channel and added an `axum`-backed `serve_sse` HTTP
server with `Last-Event-ID` replay. 17 tests pass; clippy pedantic +
`#![deny(warnings)]` + fmt clean. Added `futures-util` workspace dep.
Workspace `cargo check` green. No issues.

### Open questions / blockers
None.
