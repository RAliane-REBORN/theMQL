# SESSION.md

Current session state. Update at the end of every turn (see `MEMORY.md`
standing rules). When a session ends, fold the in-flight items into
`HANDOVER.md`.

## Current session

- Date: 2026-08-20
- Mode: build
- Agent: opencode (glm-5.2:cloud)
- Branch: `feat/phase-4-mqtt-auth-embedded` (off main, post PR #5 merge)

## Just-completed turn

Phase 4 — MQTT bridge, auth, and embedded embassy main (4 steps):

1. **Step 1 — Fix machete + merge PR #5**: Removed unused `serde_json`
   dev-dep from `themql-analysis`. Pushed, CI green (all 8 jobs), merged
   PR #5 to main (`983e529`). Created `feat/phase-4-mqtt-auth-embedded`.
2. **Step 2 — Wire MQTT bridge into serve**: Added `themql-mqtt` dep to
   `themql-desktop`. `MqttToSseBridge` (MessageHandler) re-publishes
   incoming MQTT messages to the SSE publisher via `broadcast()`.
   `--enable-mqtt`, `--mqtt-host`, `--mqtt-port`, `--mqtt-client-id`
   CLI args. Background tokio task drives rumqttc event loop.
   `serve_sse_with_publisher` variant added to themql-sse for shared
   `Arc<TokioSsePublisher>`. +3 tests.
3. **Step 3 — Auth via better-auth + MQTT creds**: Created
   `specs/auth.toml` (authn: GraphQL sessions via better-auth, MQTT
   username/password; authz: role-based ACLs). Added `better-auth` 0.10
   (axum + rustls) to workspace deps. `RumqttcConfig` gains
   `username`/`password` fields + `with_credentials()` builder.
   `--enable-auth` + `--auth-secret` (or `THEMQL_AUTH_SECRET` env var)
   wires `BetterAuth` with `MemoryDatabaseAdapter` +
   `EmailPasswordPlugin` into the serve router. `--mqtt-username`/
   `--mqtt-password` for broker authn. +5 tests.
4. **Step 4 — Real embassy embedded main**: `#![no_std]` +
   `#![no_main]` via `cfg_attr(target_os = "none")`. Embassy executor
   with `platform-cortex-m` + `executor-thread`. 7 embassy tasks per
   spec: gps (10Hz), baro (50Hz), imu (200Hz), estimator (200Hz),
   telemetry (10Hz), inference (5Hz), command. Inter-task channels via
   `embassy_sync::channel::Channel<CriticalSectionRawMutex>`.
   `HeapString` fixed-capacity (64B) string for no_std error messages.
   Panic handler with spin_loop. Host stub retained for x86 tests.
   Cross-compiles clean: `cargo check --target thumbv7em-none-eabihf`.
   New CI job: `embedded-check`. +7 tests.

**Totals**: 325 tests pass workspace-wide (was 306 at start of turn).
Full validation green: fmt, clippy, test, machete, TOML sanity, ci_guard.
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
  Actions CI, real themql-desktop subcommands. PR #5 merged to main.
- Phase 4 complete: MQTT-to-SSE bridge in serve, better-auth GraphQL
  auth + MQTT broker credentials, real embassy embedded main.
- 325 tests pass workspace-wide (default features). 20 crates, 21 specs.
- Full validation green.
- `tch-backend` feature compiles clean (tests not run: libtorch OOM).
- Embedded binary cross-compiles for thumbv7em-none-eabihf.
- No open bugs.

## In-flight work

None. Phase 4 is complete. Changes are committed on
`feat/phase-4-mqtt-auth-embedded` branch, ready to push and open PR #6.

## Next plausible actions (suggestions, not commitments)

1. Merge PR #6 to main after CI green.
2. Run `tch-backend` feature tests once a beefier environment is
   available (>7.8GB RAM).
3. Make themql-gnc/themql-estimation no_std compatible so the embedded
   binary can call real EKF + HybridController (currently embassy tasks
   stub the GNC/estimation logic).
4. GraphQL field-level authz (subject-pattern ACLs per role).
5. MQTT topic filter ACLs per client id.
6. Fuzzing harness + secret-management policy.

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
