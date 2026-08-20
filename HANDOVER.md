# HANDOVER.md

Handover notes for the next agent/session. Fold in-flight items from
`SESSION.md` here when a session ends. Update after every turn (see
`MEMORY.md` standing rules).

## Handover from: opencode (glm-5.2:cloud), 2026-08-20 (Phase 2 Stage 12 complete)

### Repository state at handover

- Phase 1 complete (spec + 20 crates implemented). Phase 2 Stages 1-12
  complete: cache/storage backends, transport adapters, runtime impls,
  desktop/embedded binary wiring, analysis/training/inference heavy-dep
  wiring, cross-crate type reconciliation, safety-critical tooling,
  opencode.json, CI guard script.
- Branch: `feat/phase-2-heavy-dep-wiring`.
- 240 tests pass workspace-wide (default features). 20 crates, 20 specs.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace`, `cargo deny check`,
  `cargo machete --with-metadata`, `scripts/ci_guard.py` all green.
- BUG-0007 resolved (no open bugs).
- Commit `d97fcca` pushed to `feat/phase-2-heavy-dep-wiring`, PR #4 open.
- Working tree clean.

### What is done this turn

- **Stage 12 — final validation + commit + PR**: ran full validation
  suite (all 7 gates green), updated all 8 living docs for Stage 12
  completion, committed all Phase 2 work as `d97fcca` (48 files, +6441/-
  960), pushed to `feat/phase-2-heavy-dep-wiring`, created PR #4.
- **Stage 11a — safety-critical tooling**: installed cargo-deny,
  cargo-machete, cargo-bloat. `cargo deny check` passes (added BSL-1.0
  + CDLA-Permissive-2.0 to deny.toml). `cargo machete` clean (removed
  11 unused deps across 8 crates). `cargo bloat` passes (3.9MiB binary).
  Removed conflicting cargo aliases from .cargo/config.toml.
- **Stage 11b — opencode.json**: wrote project config with $schema,
  instructions, permissions, and validate/safety-gate custom commands.
- **Stage 11c — CI guard script**: wrote `scripts/ci_guard.py` checking
  TOML parse + crate-name-vs-workspace invariant. All checks pass.

### Validation commands the next agent should run

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 \
  CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=none cargo test --workspace
cargo deny check
cargo machete --with-metadata
python3 scripts/ci_guard.py
```

## Handover from: opencode (glm-5.2:cloud), 2026-08-20 (Phase 2 Stage 10 complete)

### Repository state at handover

- Phase 1 complete (spec + 20 crates implemented). Phase 2 Stages 1-10
  complete: cache/storage backends, transport adapters, runtime impls,
  desktop/embedded binary wiring, analysis/training/inference heavy-dep
  wiring, cross-crate type reconciliation.
- Branch: `feat/phase-2-heavy-dep-wiring`.
- 240 tests pass workspace-wide (default features). 20 crates, 20 specs.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace` all green.
- BUG-0007 resolved (no open bugs).
- Working tree has uncommitted changes. The user has not requested
  commits.

### What is done this turn

- **Stage 10a — CacheKey reconciliation**: `themql-cache` now re-exports
  `CacheKey` from `themql-query` (per `specs/cache.toml`). Removed local
  `CacheKey` definition + `blake3` dep. Added `hash_of()` + serde
  derives to `themql-query::CacheKey`.
- **Stage 10b — themql-schema crate**: created 20th crate with canonical
  shared types matching `specs/training.toml` exactly. Fixed spec
  deviations from `themql-artifact`: `FeatureDType` has `Bool` (not
  `I32`), `NormalizationSpec` is simple enum (not struct variants),
  `FeatureSchema` has `normalization` field. 6 tests.
- **Stage 10c — updated dependent crates**: `themql-artifact`,
  `themql-training`, `themql-inference` re-export from `themql-schema`.
  Created `specs/schema.toml`. Updated spec references.

### Environment constraints

- Same as previous: 7.8GB RAM, no swap. Use
  `CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0
  CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=none` for polars-dependent builds.
- `tch-backend` feature compiles clean but tests not run (libtorch +
  RAM).

### Validation commands the next agent should run

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 \
  CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=none cargo test --workspace
```

## Handover from: opencode (glm-5.2:cloud), 2026-08-20 (Phase 2 Stages 2-5 + 9 complete)

### Repository state at handover

- Phase 1 complete (spec + 19 crates implemented). Phase 2 Stages 1-9
  complete: cache/storage backends, transport adapters (SSE/MQTT/GraphQL),
  runtime impls, desktop/embedded binary wiring, analysis/training/
  inference heavy-dep wiring.
- Branch: `feat/phase-2-heavy-dep-wiring`.
- 234 tests pass workspace-wide (default features).
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace` all green.
- BUG-0007 resolved (no open bugs).
- Working tree has uncommitted changes. The user has not requested
  commits.

### What is done this turn

- **Stage 2 — themql-sse**: added `SseEvent::to_wire_string()` (SSE
  wire format), `SseEvent::from_message()` (Message→SseEvent),
  `TokioSsePublisher` (broadcast channel-backed), `TokioSseStream`
  (broadcast receiver → SSE event stream). Added `serde_json`,
  `tokio` (rt/sync/io-util) to deps, `tokio` (macros/rt-multi-thread)
  to dev-deps. 13 tests pass.
- **Stage 3 — themql-mqtt**: added `subject_to_topic()` (identity
  mapping per spec), `topic_to_subject()`, `encode_message()`
  (Message→JSON bytes), `decode_message()` (JSON bytes→Message). Moved
  `serde_json` from dev-dep to main deps. 14 tests pass.
- **Stage 4-5 — themql-graphql**: added `#[Object]` impls for
  `QueryRoot` and `MutationRoot` (placeholder fields).
  `SubscriptionRoot` kept as marker only (no `#[Object]` —
  `SubscriptionType` trait not implemented). Tests use
  `async_graphql::EmptySubscription`. Added `tokio` (macros) to
  dev-deps. 6 tests pass.
- **Stage 9 — themql-analysis**: added `PolarsAnalysisResult`,
  `PolarsDatasetBuilder` (constructs `DataFrame` from headers + rows),
  `RayonAnalysisPipeline` (parallel null-count via `par_iter`). Added
  `polars` and `rayon` to deps. 9 tests pass.
- **Stage 9 — themql-training**: added `TchTrainer` implementing
  `Trainer` behind `cfg(feature = "tch-backend")`. Added `tch`
  (optional, `download-libtorch`) and `rayon` to deps; added
  `[features] tch-backend = ["dep:tch"]`. 5 tests pass (default
  features).
- **Stage 9 — themql-inference**: added `TchInferenceEngine`
  implementing `InferenceEngine` behind `cfg(feature = "tch-backend")`.
  Added `tch` (optional, `download-libtorch`) to deps; added
  `[features] tch-backend = ["dep:tch"]`. 6 tests pass (default
  features).

### Environment constraints

- The `tch-backend` feature in themql-training/themql-inference
  compiles clean but tests are NOT run in this environment. libtorch
  download (~174MB) + 7.8GB RAM (no swap) make the full tch test build
  impractical. polars-core alone OOM-kills rustc without
  `CARGO_PROFILE_DEV_DEBUG=0`.
- To build/test polars-dependent crates in this environment, use:
  `CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0
  CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=none cargo test ...`
- Full workspace test takes ~15min with these settings.

### Stubs / follow-ups introduced this turn

- `tch-backend` feature tests (`TchTrainer`, `TchInferenceEngine`)
  require libtorch + more RAM to run.
- Transport adapters (SSE/MQTT/GraphQL) are real but minimal — they
  prove the trait surface compiles with the heavy deps; real network
  I/O is a future task.
- `RayonAnalysisPipeline` runs a trivial parallel null-count; real
  analysis pipelines are a future task.
- `TchTrainer` runs a placeholder training loop (tensor creation +
  `tanh`); real training is a future task.
- `TchInferenceEngine` runs a dummy forward pass; real model loading
  is a future task.

### Validation commands the next agent should run

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 \
  CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=none cargo test --workspace
```

The `tch-backend` feature compiles clean:
```
cargo clippy -p themql-training -p themql-inference \
  --features tch-backend --all-targets -- -D warnings
```

## Handover from: opencode (glm-5.2:cloud), 2026-08-20 (Phase 2 Stages 6-8 complete)

### Repository state at handover

- Phase 1 complete (spec + 19 crates implemented). Phase 2 Stage 1
  complete: themql-cache backends (L1/L2/L3 + TieredCache) and
  themql-storage backend (HelixStorage in-memory) wired.
- Branch: `feat/phase-2-heavy-dep-wiring`.
- 59 tests pass across the two crates in scope (30 themql-cache + 29
  themql-storage).
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` all green for the two crates in scope.
- Working tree has uncommitted changes. The user has not requested
  commits.

### What is done this turn

- **themql-cache backends**: added `L1Cache` (bounded `HashMap` + FIFO
  eviction — cachelito v0.16 API is macro/`'static`-singleton-based and
  unsuitable for per-instance `CacheKey`-keyed L1, so the spec/`HashMap`
  fallback was used per task brief), `L2Cache` (`moka::sync::Cache`),
  `L3Cache` (`valkey` 0.0.0-alpha5 stub — driver lacks `DEL`/`EXPIRE`/
  binary, ops return `TierUnavailable`), and `TieredCache<S: Storage>`
  orchestrator implementing `Cache`: `get` walks L1→L2→L3→L4 promoting
  on hit; `put` writes through; `invalidate` removes from all;
  `invalidate_pattern` scans L1/L2 keys (best-effort). +14 tests, 30
  total passing.
- **themql-storage backend**: added `HelixStorage` implementing
  `Storage` — in-memory `HashMap` fallback (helix-db v3.0 is async HTTP
  for a live server, graph-DSL not KV); real adapter
  FOLLOW-UP_REQUIRED. `query` supports `ByKey` + `BySubjectPattern`;
  `ByPredicate` returns `QueryError`. +6 tests, 29 total passing.

### Stubs / follow-ups introduced this turn

- L1 cachelito integration deferred (v0.16 API mismatch).
- L3 valkey integration deferred (alpha driver lacks required ops).
- L4 helix-db integration deferred (requires live server + KV mapping).
- `invalidate_pattern` against L1/L2 is effectively a no-op until a
  key→subject index is added (`CacheKey` is an opaque BLAKE3 hash).

### Validation commands the next agent should run

- `cargo fmt -p themql-cache -p themql-storage --check`
- `cargo clippy -p themql-cache -p themql-storage --all-targets -- -D warnings`
- `cargo test -p themql-cache -p themql-storage`

## Handover from: opencode (glm-5.2:cloud), 2026-08-20 (Phase 2 Stages 6-8 complete)

### Repository state at handover

- Phase 1 complete (spec + 19 crates implemented). Phase 2 Stages 6-8
  complete: runtime impls + desktop binary + embedded binary wired with
  real deps (tokio, ratatui/crossterm) and real tests.
- Branch: `feat/phase-2-heavy-dep-wiring`.
- 29 default-feature tests + 8 desktop-feature runtime tests pass across
  the three crates in scope (themql-runtime, themql-desktop,
  themql-embedded).
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` all green for the three crates, both with and without the
  `desktop`/`embedded` feature flags.
- **BLOCKER**: `cargo check --workspace` currently fails in
  `themql-cache` and `themql-storage` due to **pre-existing uncommitted
  changes** in those crates that were in the working tree before this
  turn began (confirmed via `git stash`). Those are out of scope for
  Stages 6-8 and not introduced by this turn. The next agent should fix
  or revert those before running the workspace-wide gate.
- Working tree has uncommitted changes (mine + the pre-existing
  cache/storage ones). The user has not requested commits.

### What is done this turn

- **Stage 6 — themql-runtime**: verified `TokioRuntime` (desktop) and
  `EmbassyRuntime` (embedded) both compile-check clean; added 8 real
  `TokioRuntime` tests behind `cfg(feature = "desktop")` (spawn result,
  sleep duration, timeout fires/succeeds, channel send/recv,
  cancellation cancel-then-observe + already-cancelled-resolves, spawn
  join error on panic). Added a `[dev-dependencies]` tokio with
  `rt-multi-thread` + `test-util` + `macros` so the tests can build a
  runtime. Clippy clean on both `--features desktop` and
  `--features embedded`.
- **Stage 7 — themql-desktop (binary)**: added tokio + ratatui deps;
  rewrote `main.rs` with `#[tokio::main]`, clap CLI dispatch (per-command
  banners), a real `tui_main()` using `ratatui::init`/`restore` +
  crossterm event polling (quit on `q`/`Esc`), a 3-pane dashboard
  (`draw_dashboard`: telemetry stream / state estimate / controller
  state per `specs/desktop.toml [api.TuiLayout]`), and a `TestBackend`
  unit test for the draw. 13 tests pass. ratatui 0.30 re-exports
  crossterm via its default `crossterm` feature, so no separate
  `crossterm` crate dep was needed.
- **Stage 8 — themql-embedded (binary)**: the existing stub already had
  `SensorKind`, `SensorError` (5 variants), `SensorDriver` trait,
  `GpsDriver`/`BaroDriver`/`ImuDriver`. Updated the main stub banner to
  "embassy runtime requires thumbv7em target"; added 6 more tests
  (per-driver construction, all-variants-distinct, trait-object
  dispatch, `SensorError: std::error::Error`). 10 tests pass. No embassy
  added (won't compile on x86 host, per spec).

### Stubs / follow-ups introduced this turn

- `themql-embedded` main is a stub (`embassy runtime requires thumbv7em
  target`). The real `#[embassy_executor::main]` is a future task per
  `specs/embedded.toml [entry]`.
- `themql-desktop` dispatch prints banners; real serve/analyze/train/
  validate/telemetry implementations are future tasks. The TUI is real
  (renders + handles quit keys) but panes are empty placeholders pending
  telemetry wiring.

## Handover from: opencode (glm-5.2:cloud), 2026-08-20 (Phase 1 complete)

### Repository state at handover

- v0.1 specification drop + workspace skeleton + living-docs + AGENTS.md +
  subsystem spec set + Phase 1 dep wiring + toolchain hardening + Phase 1
  complete. ALL 19 crates now have real `src/` content.
- Branch: `feat/phase-1-spec-deepening` (stacked on
  `feat/v0.1-spec-and-workspace-skeleton`, PR #1 open).
- 19 crates, 43+ TOML files, all parsing.
- 183 unit tests + 1 doc test = 184 tests, all pass.
- Full validation green: `cargo fmt --all --check`, `cargo check --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings` (zero warnings),
  `cargo test --workspace` (184 pass), `python3 tomllib` (all parse),
  `cargo metadata --no-deps` (resolves).
- Build toolchain config in place: rust-toolchain.toml (components + targets),
  .cargo/config.toml (mold/sccache opt-in), deny.toml (cargo-deny),
  TETANUS.md (Power of 10).
- Working tree has uncommitted changes. The user has not requested commits.

### What is done

- **nalgebra amendment** — nalgebra 0.35 replaces ndarray for non-ML
  numerics. Updated SYSTEM.md, ARCHITECT.md, SPEC.toml, Cargo.toml,
  AGENTS.md, MEMORY.md.
- **Stage 1 — spec deepening** — ALL 19 specs deepened from thin/medium to
  deep with concrete types/traits/signatures. User decisions applied:
  Selection = filter + projection combined (core.toml); TWO separate
  runtime traits (DesktopRuntime tokio + EmbeddedRuntime embassy), no
  shared trait (runtime.toml); EstimatorState = 21-dim
  (state_estimation.toml).
- **Stage 2 — all 19 crates implemented** — each crate has real Rust
  types/traits + unit tests. Per-crate test counts:

  | Crate | Tests | Notes |
  |---|---|---|
  | themql-core | 31 + 1 doc | canonical types + Resolver/MessageHandler/QueryExecutor traits |
  | themql-message | 4 | Serializer + JsonSerializer + MessageError |
  | themql-query | 11 | CacheKey, CacheKeyer, Batcher, QueryError |
  | themql-runtime | 6 | DesktopRuntime + EmbeddedRuntime (separate traits) |
  | themql-cache | 16 | Cache trait, CacheEntry, CacheHit, CacheError |
  | themql-storage | 23 | Storage/Reader/Writer traits, StorageError |
  | themql-transport | 14 | Bridge trait, BridgeRoute, TransportError |
  | themql-telemetry | 14 | TelemetryMessage, sensor structs, Covariance [f64;441] |
  | themql-mqtt | 9 | MqttTransport/Publisher/Subscriber, MqttQos, SubscriptionId |
  | themql-graphql | 4 | GraphqlSchema/GraphqlResolverBridge, Query/Mutation/Subscription roots |
  | themql-sse | 8 | SseStream/SsePublisher, SseEvent, SseError |
  | themql-artifact | 5 | ArtifactValidator/Loader/Writer, ModelArtifact. TETANUS-compliant |
  | themql-gnc | 7 | Controller trait, PID/LQRI/Hybrid, GncState 21-dim. TETANUS-compliant |
  | themql-estimation | 6 | Estimator trait, Ekf, EstimatorState 21-dim. TETANUS-compliant |
  | themql-inference | 6 | InferenceEngine, ResourceBudget, RollbackHandle. TETANUS-compliant |
  | themql-training | 5 | Trainer trait, Dataset, TrainingConfig, TrainedModel |
  | themql-analysis | 5 | AnalysisPipeline, DatasetBuilder, ThedafAdapter |
  | themql-desktop | 5 | Cli (clap), Command enum, main entry |
  | themql-embedded | 4 | SensorDriver, SensorError, main stub. TETANUS-compliant |

- **New workspace deps added this turn**: nalgebra 0.35, uuid 1 (v7+serde),
  blake3 1, thiserror 2, serde-big-array 0.5 (for [f64; 441] covariance serde).
- `SPEC.toml` (v0.1) with the training/inference split applied (19 crates).
- 19 `specs/*.toml` subsystem specs (every crate has a deep matching spec).
- 6 `prompts/*.md` role files (SYSTEM, ARCHITECT, IMPLEMENTER, REVIEWER,
  RED_TEAM, TEST_ENGINEER).
- `Cargo.toml` workspace root with resolved dep versions + `[profile.dev]`,
  `[profile.release]`, `[profile.bench]` + `rust-toolchain.toml`.
- `.cargo/config.toml` — mold linker + sccache wrapper configs (opt-in).
  Cargo aliases: chk, tst, fmtc, clip, deny, machete.
- `deny.toml` — cargo-deny config (licenses, advisories, bans, sources).
- `TETANUS.md` — NASA JPL Power of Ten adapted for Rust. 10 rules with
  enforcement mechanisms. Safety-critical crates: themql-gnc,
  themql-estimation, themql-inference, themql-artifact.
- 8 living docs at repo root (MEMORY, CHANGELOG, SESSION, HANDOVER,
  SECURITY, BUGS, AGENTS_SYNC, README).
- `AGENTS.md` — OpenCode convention file with safety-critical validation
  section and TETANUS.md summary.

### What is not done

- **Real nonlinear quaternion EKF dynamics + Jacobian-based Kalman gain**
  in themql-estimation. v0.1 uses simplified identity-gain updates as a
  placeholder. Must replace before flight use.
- **Real SHA-256 (or BLAKE3)** in themql-artifact's HashValidator. v0.1 uses
  a placeholder fold hash (deterministic but NOT cryptographically secure).
  Must replace before deployment.
- **Concrete tch-backed InferenceEngine impl** in themql-inference. v0.1
  defines the trait only; `tch` not added to avoid the libtorch build dep.
- **Heavy deps wired into training/analysis/desktop/embedded**. v0.1 defines
  traits + minimal types only; tch, polars, dioxus, ratatui, embassy are
  deferred. Switch local type stubs (FeatureSchema, ModelFormat,
  ValidationMetrics, Dataset, ImuReading/GpsReading/BarometerReading) to
  re-exports from canonical owners (themql-artifact, themql-telemetry) when
  those crates grow concrete impls.
- **EmbassyRuntime sleep** in themql-runtime is a stub; embassy 0.10
  Spawner is not Send/Sync so EmbeddedRuntime trait was relaxed from spec.
- **CacheKey reconciliation**: defined locally in themql-cache (BLAKE3
  32-byte); reconcile with themql-query's CacheKeyer later — do not
  duplicate silently.
- Concrete cache backends (cachelito L1, moka L2, valkey L3) behind the
  `Cache` trait.
- Concrete helix-db L4 adapter behind the `Storage` trait.
- Concrete MQTT/GraphQL/SSE transport adapters behind the `Bridge` trait.
- CI guard script.
- `opencode.json` — not yet written.
- `mold`, `sccache`, `cargo-deny`, `cargo-machete`, `cargo-bloat` not
  installed in this environment. Config files are ready for when they are.
  Install: `apt install mold`, `cargo install sccache cargo-deny cargo-machete
  cargo-bloat`. Then uncomment the relevant lines in `.cargo/config.toml`.
- Reassessment of `cachelito` (proc-macro for function caching — may not
  match L1 use case) and `valkey` (alpha version). Note: `helix-db` v3.0.0
  compiles cleanly in this environment.

### Standing rules the next agent MUST follow

1. Authority hierarchy: `SPEC.toml` → `specs/*.toml` → tests → existing
   public APIs → dependency docs → assumptions. Never silently override.
2. Composition over reimplementation. Use the listed deps; do not rebuild
   them.
3. `themql-ai` does not exist. Model creation is `themql-training` (desktop);
   model execution is `themql-inference` (embedded). Artifact validation is
   `themql-artifact`. Boundary: `specs/model_artifact.toml`.
4. GNC authority: basic flight stability must work without ML. ML may
   augment the state estimate; it must not silently replace the deterministic
   controller.
5. Quaternion is canonical attitude. Euler angles only for UI/logging.
6. `tch` (published as `tch`, not `tch-rs`) is forbidden in the GNC control
   loop (`specs/gnc.toml: tch_rs_in_control_loop = false`).
7. theDAF is integration-only. Not a core dep. Not an embedded dep.
8. Living-docs rule: after every turn, update README, SECURITY, SESSION,
   HANDOVER, CHANGELOG, BUGS, MEMORY, AGENTS_SYNC. A turn is not finished
   while any of these is stale.
9. Do not commit unless the user explicitly asks.
10. `CacheKey` lives in `themql-cache` (locally defined, BLAKE3 32-byte),
    not re-exported from `themql-query`. When `themql-query` implements its
    own `CacheKeyer`, reconcile the two — do not duplicate the key type
    silently.

### Suggested next actions (confirm with user first)

1. Decide commit strategy.
2. Implement full nonlinear quaternion EKF dynamics + Jacobian-based
   Kalman gain in themql-estimation (replace the v0.1 identity-gain
   placeholder).
3. Wire a real SHA-256 (or BLAKE3) into themql-artifact's HashValidator
   (replace the placeholder fold hash — required before deployment).
4. Add a concrete `tch`-backed `InferenceEngine` impl in themql-inference
   once libtorch is available in the build env.
5. Reconcile CacheKey between themql-cache and themql-query.
6. Wire concrete backends: cachelito L1 / moka L2 behind the `Cache` trait;
   helix-db L4 behind the `Storage` trait; MQTT/GraphQL/SSE behind the
   `Bridge` trait.
7. Wire heavy deps (tch, polars, dioxus, ratatui, embassy) into the
   training/analysis/desktop/embedded crates.
8. Add a CI guard script (TOML parse + crate-name-vs-workspace invariant).
9. Reassess `cachelito` vs L1 use case. Reassess `valkey` alpha stability.
10. (optional) Write `opencode.json`.

### Open questions / blockers

None. Awaiting user direction.

### Validation commands the next agent should run

- TOML parse: `python3 -c "import tomllib, pathlib; [tomllib.loads(p.read_text()) for p in pathlib.Path('.').rglob('*.toml') if '.git' not in p.parts]"`
- Workspace check: `cargo metadata --no-deps --format-version 1 > /dev/null`
- Full gate: `cargo fmt --all --check && cargo check --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
- Safety-critical crates (once cargo-deny/machete/bloat installed):
  `cargo deny check`, `cargo machete --workspace`,
  `cargo bloat --release --crates`,
  `cargo +nightly miri test` (if any unsafe code exists — none does currently).

---

## Handover from: opencode (glm-5.2:cloud), 2026-08-20 (earlier turn)

### Repository state at handover

- v0.1 specification drop + workspace skeleton + living-docs + AGENTS.md +
  subsystem spec set + Phase 1 dep wiring + toolchain hardening + Phase 1
  cross-cutting crates (cache/transport/storage) + four safety-critical
  crates (artifact/gnc/estimation/inference) all complete and validated.
- 19 crates, 43+ TOML files, all parsing.
- 9 crates now have real `src/lib.rs` content: themql-core, themql-message,
  themql-cache, themql-transport, themql-storage, themql-artifact,
  themql-gnc, themql-estimation, themql-inference.
- For the four new safety-critical crates: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test` all pass
  (24/24 tests green). All four carry `#![forbid(unsafe_code)]` +
  `#![deny(warnings)]` + `clippy::pedantic`.
- Build toolchain config in place: rust-toolchain.toml (components + targets),
  .cargo/config.toml (mold/sccache opt-in), deny.toml (cargo-deny),
  TETANUS.md (Power of 10).
- Working tree has uncommitted changes. The user has not requested commits.

### What is done

- `SPEC.toml` (v0.1) with the training/inference split applied (19 crates).
- 19 `specs/*.toml` subsystem specs (every crate has a matching spec).
  - `core.toml` is canonical for Message/Query/Response/Error/Context/Resource.
  - `message.toml` and `query.toml` are subordinate to `core.toml`.
  - `transport.toml` is cross-cutting; `mqtt.toml`, `graphql.toml`, `sse.toml`
    are subordinate sub-specs.
  - `model_artifact.toml` is deep, with `crate = "themql-artifact"`.
- 6 `prompts/*.md` role files (SYSTEM, ARCHITECT, IMPLEMENTER, REVIEWER,
  RED_TEAM, TEST_ENGINEER).
- `Cargo.toml` workspace root with resolved dep versions + `[profile.dev]`,
  `[profile.release]`, `[profile.bench]` + `rust-toolchain.toml` (stable
  channel, components: rustfmt, clippy, rust-src, miri; targets: x86_64,
  thumbv7em, riscv32imc).
- `.cargo/config.toml` — mold linker + sccache wrapper configs (opt-in via
  comments). Cargo aliases: chk, tst, fmtc, clip, deny, machete.
- `deny.toml` — cargo-deny config (licenses, advisories, bans: pyo3/cpython/
  python3-sys/rusqlite, sources: crates.io only).
- `TETANUS.md` — NASA JPL Power of Ten adapted for Rust. 10 rules with
  enforcement mechanisms. Safety-critical crates identified: themql-gnc,
  themql-estimation, themql-inference, themql-artifact.
- 19 crate stubs (17 libs + 2 binaries). 7 crates have real deps wired:
  themql-core (serde, serde_json), themql-storage (helix-db, serde,
  themql-core, thiserror, serde_json), themql-graphql (async-graphql,
  themql-core, themql-query), themql-mqtt (embassy-executor, embassy-sync,
  themql-core, themql-message), themql-sse (tokio, themql-core,
  themql-message), themql-artifact (serde). themql-cache (themql-core,
  thiserror, blake3, serde) and themql-transport (themql-core,
  themql-message, thiserror, serde) wired this turn.
- Real `src/lib.rs` content in 9 crates:
  - `themql-core` — canonical types (Message, Query, Response, Error,
    Context, Resource, Subject, SubjectPattern, CachePolicy, CacheTier,
    Resolver/MessageHandler/QueryExecutor traits).
  - `themql-message` — Serializer trait + JsonSerializer + MessageError.
  - `themql-cache` — CacheKey (BLAKE3), CacheEntry, CacheHit, Cache trait,
    CacheError. 16 tests.
  - `themql-transport` — TransportKind, SubjectRewrite, BridgeRoute,
    Bridge trait, TransportError. 14 tests.
  - `themql-storage` — StorageKey, StorageValue, StorageQuery,
    StorageResultSet, Storage/StorageReader/StorageWriter traits,
    StorageError. 23 tests.
  - `themql-artifact` — ModelFormat, FeatureSchema/FeatureSpec/
    FeatureDType, NormalizationSpec, ValidationMetrics, PruningMetadata,
    TensorSchema, CompatibilityInfo, ArtifactMetadata, TrainedModel,
    ModelArtifact, ValidationReport, RuntimeInfo, ActivationHandle,
    ArtifactError, ArtifactValidator/ArtifactLoader/ArtifactWriter
    traits, HashValidator reference impl. 5 tests. TETANUS-compliant.
  - `themql-gnc` — GncState (21-dim nalgebra UnitQuaternion), Setpoint,
    ActuatorCommand ([f64;8]), PidController (windup saturation),
    LqriController (SMatrix 8×21/8×3/21×21/8×8), HybridController
    (function-pointer Schedule policy), Controller trait, GncError.
    7 tests. TETANUS-compliant.
  - `themql-estimation` — STATE_DIM=21, EstimatorState (SVector/SMatrix),
    ImuReading/GpsReading/BarometerReading (local stubs),
    Estimator trait, Ekf (v0.1 simplified identity-gain updates),
    EstimationError. 6 tests. TETANUS-compliant.
  - `themql-inference` — re-exports from themql-artifact,
    ResourceBudget, RollbackHandle, InferenceInput/Output (21-dim
    nalgebra state), InferenceEngine trait, InferenceError. 6 tests.
    TETANUS-compliant. `tch` deliberately not added (libtorch build dep).
- 8 living docs at repo root (MEMORY, CHANGELOG, SESSION, HANDOVER,
  SECURITY, BUGS, AGENTS_SYNC, README).
- `AGENTS.md` — OpenCode convention file with safety-critical validation
  section and TETANUS.md summary.
- Validation: 43/43 TOML parse, `cargo check --workspace` passes for all 19
  crates with real deps from crates.io. The four new safety-critical
  crates pass fmt/clippy/test with zero warnings (pedantic +
  deny(warnings)) and 24/24 tests.

### What is not done

- Real Rust source in the remaining 10 crates — Phase 1 continues.
  themql-query, themql-runtime, themql-graphql, themql-mqtt, themql-sse,
  themql-telemetry, themql-analysis, themql-training, themql-embedded,
  themql-desktop still have 0-line stubs (or local stub types that should
  re-export from canonical owners once those land).
- Concrete cache backends (cachelito L1, moka L2, valkey L3) behind the
  `Cache` trait.
- Concrete helix-db L4 adapter behind the `Storage` trait.
- Concrete MQTT/GraphQL/SSE transport adapters behind the `Bridge` trait.
- Real `tch`-backed `InferenceEngine` impl in themql-inference (v0.1
  defines the trait only; tch not added to avoid libtorch build dep).
- Full nonlinear quaternion EKF dynamics + Jacobian-based Kalman gain in
  themql-estimation (v0.1 uses simplified identity-gain updates).
- Real SHA-256 in themql-artifact's HashValidator (v0.1 uses a
  placeholder fold hash to avoid a crypto dep in this safety-critical
  crate).
- Canonical sensor reading types in themql-telemetry (estimation
  currently defines local ImuReading/GpsReading/BarometerReading stubs).
- CI guard script.
- `opencode.json` — not yet written.
- `mold`, `sccache`, `cargo-deny`, `cargo-machete`, `cargo-bloat` not
  installed in this environment. Config files are ready for when they are.
  Install: `apt install mold`, `cargo install sccache cargo-deny cargo-machete
  cargo-bloat`. Then uncomment the relevant lines in `.cargo/config.toml`.
- Reassessment of `cachelito` (proc-macro for function caching — may not
  match L1 use case) and `valkey` (alpha version). Note: `helix-db` v3.0.0
  compiles cleanly in this environment.

### Standing rules the next agent MUST follow

1. Authority hierarchy: `SPEC.toml` → `specs/*.toml` → tests → existing
   public APIs → dependency docs → assumptions. Never silently override.
2. Composition over reimplementation. Use the listed deps; do not rebuild
   them.
3. `themql-ai` does not exist. Model creation is `themql-training` (desktop);
   model execution is `themql-inference` (embedded). Artifact validation is
   `themql-artifact`. Boundary: `specs/model_artifact.toml`.
4. GNC authority: basic flight stability must work without ML. ML may
   augment the state estimate; it must not silently replace the deterministic
   controller.
5. Quaternion is canonical attitude. Euler angles only for UI/logging.
6. `tch` (published as `tch`, not `tch-rs`) is forbidden in the GNC control
   loop (`specs/gnc.toml: tch_rs_in_control_loop = false`).
7. theDAF is integration-only. Not a core dep. Not an embedded dep.
8. Living-docs rule: after every turn, update README, SECURITY, SESSION,
   HANDOVER, CHANGELOG, BUGS, MEMORY, AGENTS_SYNC. A turn is not finished
   while any of these is stale.
9. Do not commit unless the user explicitly asks.
10. `CacheKey` lives in `themql-cache` (locally defined, BLAKE3 32-byte),
    not re-exported from `themql-query`. When `themql-query` implements its
    own `CacheKeyer`, reconcile the two — do not duplicate the key type
    silently.

### Suggested next actions (confirm with user first)

1. Decide commit strategy.
2. Continue Phase 1: real `src/lib.rs` content in `themql-query`
   (CacheKeyer, QueryExecutor). The cache crate avoided depending on it;
   reconcile `CacheKey` when query lands.
3. Wire a concrete cache backend (cachelito L1, moka L2) behind the
   `Cache` trait.
4. Wire the helix-db L4 adapter behind the `Storage` trait.
5. Implement MQTT/GraphQL/SSE transport adapters behind the `Bridge`
   trait.
6. Add a CI guard script (TOML parse + crate-name-vs-workspace invariant).
7. Reassess `cachelito` vs L1 use case. Reassess `valkey` alpha stability.
8. (optional) Write `opencode.json`.

### Open questions / blockers

None. Awaiting user direction.

### Validation commands the next agent should run

- TOML parse: `python3 -c "import tomllib, pathlib; [tomllib.loads(p.read_text()) for p in pathlib.Path('.').rglob('*.toml') if '.git' not in p.parts]"`
- Workspace check: `cargo metadata --no-deps --format-version 1 > /dev/null`
- Full check: `cargo check --workspace`
- Safety-critical crates: `cargo fmt -p themql-artifact -p themql-gnc -p themql-estimation -p themql-inference --check`,
  `cargo clippy -p themql-artifact -p themql-gnc -p themql-estimation -p themql-inference --all-targets -- -D warnings`,
  `cargo test -p themql-artifact -p themql-gnc -p themql-estimation -p themql-inference`
- Once real source exists elsewhere: `cargo fmt --check`,
  `cargo clippy --workspace`, `cargo test --workspace`
---

## 2026-08-20 — handover after training/analysis/desktop/embedded stubs

### What the next agent should know

- 4 crates implemented this turn: `themql-training`,
  `themql-analysis`, `themql-desktop` (binary), `themql-embedded`
  (binary). All pass fmt + clippy (-D warnings) + test (19 tests) in
  isolation. See CHANGELOG.md / MEMORY.md 2026-08-20.
- `themql-artifact`, `themql-telemetry`, `themql-gnc`,
  `themql-estimation` still have empty `src/lib.rs`. The 4 new crates
  define dependent types (FeatureSchema, ModelFormat, ValidationMetrics,
  Dataset) locally per the task brief — when those crates are
  implemented, switch the local definitions to re-exports.
- `themql-mqtt` has pre-existing uncommitted breakage (missing
  `#[derive(Error)]` on `MqttError`). `cargo check --workspace` is red
  because of it. The 4 new crates were NOT the cause and were not
  touched. Fix `themql-mqtt` first to restore a green workspace.
- `clap` workspace dep is `clap = "4"` with NO features. Consumers
  needing derive must add `features = ["derive"]` at the use site
  (themql-desktop does this).

### Validation commands the next agent should run

- 4 new crates: `cargo fmt -p themql-training -p themql-analysis -p
  themql-desktop -p themql-embedded --check`,
  `cargo clippy -p themql-training -p themql-analysis -p themql-desktop
  -p themql-embedded --all-targets -- -D warnings`,
  `cargo test -p themql-training -p themql-analysis -p themql-desktop -p
  themql-embedded`.
- Whole workspace: fix `themql-mqtt` first, then `cargo check
  --workspace`.
