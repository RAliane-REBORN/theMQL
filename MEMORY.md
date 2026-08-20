# MEMORY.md

Living memory for theMQL. Persistent facts and rules that must survive across
sessions and across agents. Update after every turn.

## Standing rules

### Living-docs rule (added 2026-08-19)

After **every turn**, update the living docs at the repository root:

- `README.md` — project entry point
- `SECURITY.md` — security policy and threat model
- `SESSION.md` — current session state and in-flight work
- `HANDOVER.md` — handover notes for the next agent/session
- `CHANGELOG.md` — chronological record of changes
- `BUGS.md` — known bugs and unresolved issues
- `MEMORY.md` — this file
- `AGENTS_SYNC.md` — coordination log when subagents or multiple agents are
  running (leave a "no subagents this turn" entry otherwise)

These are not historical artifacts. They are working state. If a turn ends
and any of these are stale, the turn is not finished.

### Authority hierarchy (from SPEC.toml)

1. `SPEC.toml`
2. `specs/*.toml`
3. tests
4. existing public APIs
5. dependency documentation
6. implementation assumptions

Never silently override a specification.

### Composition over reimplementation

theMQL is a composition system. If a mature dependency already provides the
required primitive, use that primitive. Do not recreate it. See
`prompts/SYSTEM.md` for the full dependency policy.

### Training/inference split (v0.1)

`themql-ai` does not exist and must not be introduced. Model creation lives in
`themql-training` (desktop); model execution lives in `themql-inference`
(embedded). The artifact boundary is governed by `specs/model_artifact.toml`
and enforced by `themql-artifact` (validation crate).

### GNC authority

Basic vehicle stability must remain possible without machine learning. The
authority hierarchy is: deterministic GNC → EKF/state estimation → Bayesian
reasoning → ML-assisted state correction → ML prediction/anomaly detection. ML
must not silently become the authoritative flight-control path.

### Canonical attitude representation

Quaternion. Euler angles may be generated for UI/logging/debugging only.

### tch-rs in control loop

Forbidden. `specs/gnc.toml` enforces `tch_rs_in_control_loop = false`. This is
deliberate, not an oversight.

## Persistent facts

### Project

- Name: theMQL (The Message Query Language)
- Version: 0.1.0
- Status: greenfield (Phase 4 complete — Phase 3 Stages 1-11 + Phase 3 followups + Phase 4: MQTT-to-SSE bridge in serve, better-auth GraphQL auth + MQTT broker credentials, real embassy embedded main with 7 tasks cross-compiling for thumbv7em-none-eabihf)
- Language: Rust
- License: MIT
- Repository: https://github.com/RAliane-REBORN/theMQL
- Toolchain: stable channel, no version pin
- Workspace resolver: 2
- Branch: `feat/phase-4-mqtt-auth-embedded` (off main, post PR #5 merge)

### Crates (20)

18 library crates + 2 binary crates. ALL 20 now have real `src/` content
(traits + types + error types + unit tests). No 0-line stubs remain.
`themql-schema` is the canonical home for shared schema types
(`FeatureSchema`, `NormalizationSpec`, `ModelFormat`, etc.) used by
`themql-artifact`, `themql-training`, and `themql-inference`.

| Crate | Domain | Role | Tests |
|---|---|---|---|
| themql-core | core | semantic owner — canonical Message, Query, Response, Error, Context, Resource types + traits | 31 + 1 doc |
| themql-schema | core | canonical shared schema types (FeatureSchema, NormalizationSpec, ModelFormat, TrainedModel, etc.) | 6 |
| themql-message | core | Serializer trait + JsonSerializer + MessageError | 4 |
| themql-query | core | CacheKey, CacheKeyer, QueryExecutor, Batcher, QueryError | 11 |
| themql-runtime | runtime | DesktopRuntime (tokio) + EmbeddedRuntime (embassy), separate traits | 6 default + 8 desktop-feature |
| themql-cache | cache | Cache trait, CacheEntry, CacheHit, CacheError | 16 |
| themql-storage | storage | Storage/Reader/Writer traits, StorageKey/Value/Query/ResultSet | 23 |
| themql-transport | transport | Bridge trait, BridgeRoute, TransportKind, TransportError | 14 |
| themql-graphql | transport | GraphqlSchema/GraphqlResolverBridge traits, Query/Mutation/Subscription roots | 4 |
| themql-mqtt | transport | MqttTransport/Publisher/Subscriber traits, MqttQos, SubscriptionId | 9 |
| themql-sse | transport | SseStream/SsePublisher traits, SseEvent, SseError | 8 |
| themql-telemetry | telemetry | TelemetryMessage, sensor structs (GPS/Baro/IMU), StateEstimate, Covariance [f64;441] | 14 |
| themql-analysis | desktop | AnalysisPipeline trait, DatasetBuilder, ThedafAdapter | 5 |
| themql-training | desktop | Trainer trait, Dataset, TrainingConfig, TrainedModel | 5 |
| themql-inference | embedded | InferenceEngine trait, ResourceBudget, RollbackHandle (TETANUS) | 6 |
| themql-artifact | cross-cutting | ArtifactValidator/Loader/Writer, ModelArtifact (TETANUS) | 5 |
| themql-gnc | embedded | Controller trait, PID/LQRI/Hybrid, GncState 21-dim (TETANUS) | 7 |
| themql-estimation | embedded | Estimator trait, Ekf, EstimatorState 21-dim (TETANUS) | 6 |
| themql-embedded | binary | SensorDriver trait, GpsDriver/BaroDriver/ImuDriver, SensorError 5 variants (TETANUS) | 10 |
| themql-desktop | binary | Cli (clap), tokio main, ratatui/crossterm TUI dashboard, 3-pane layout | 13 |

Total: 304 unit tests + 1 doc test pass workspace-wide (default
features) at the Phase 3 close. Per-crate counts: analysis 12,
artifact 22, cache 32, core 31, estimation 24, gnc 7, graphql 13,
inference 6, message 4, mqtt 25, query 11, runtime 6, schema 6,
sse 17, storage 31, telemetry 14, training 6, transport 14,
desktop 13, embedded 10. The `tch-backend` feature in
training/inference compiles clean and contains a real training loop
(themql-training: MLP + Adam + MSE + TorchScript export) and a real
model loading + forward pass (themql-inference: CModule load +
forward_ts + deadline check + rollback); tests are not run
(libtorch + RAM constraints in this environment).

### Phase 3 progress (2026-08-20) — ALL STAGES COMPLETE

- Stages 1-5 — real backends wired: themql-artifact (real BLAKE3
  HashValidator, FileArtifactLoader, BincodeArtifactWriter), themql-storage
  (SledStorage disk-backed + HelixStorage alias + ByPredicate query),
  themql-cache (L1 lru, L2 moka, L3 redis, key→subject index, demotion),
  themql-estimation (real quaternion EKF with Jacobian + Joseph-form
  Kalman gain, SensorModel trait, GpsModel, BaroModel, BayesianEstimator),
  themql-analysis (polars-backed types, StorageAnalysisPipeline).
- Stages 6 & 7 — real training loop + real inference forward pass
  (behind `tch-backend`). themql-training: `TchTrainer::train` builds
  an MLP, trains with Adam + MSE for `config.epochs` (optional early
  stopping), exports to `TorchScript` bytes via
  `CModule::create_by_tracing` + temp-file `save`/read-back. The
  `Dataset` type re-exports the polars-backed
  `themql_analysis::Dataset` (added `polars` + `themql-analysis` deps).
  themql-inference: `TchInferenceEngine::load()` runs
  `HashValidator::validate` + rejects on errors + rejects empty bytes
  on every load + deserialises into a `tch::CModule` pre-allocated at
  activation; `infer()` runs the real forward pass, extracts the
  output to a 21-dim `state_correction`, checks
  `inference_deadline_ms`, populates `confidence` from the output
  norm; `rollback()` reloads the previous bytes into a fresh
  `CModule`.
- Stage 8 — real SSE server: `TokioSsePublisher` (broadcast channel +
  bounded event log), `TokioSseStream` (broadcast receiver → SSE
  event stream), `serve_sse` (axum router with `GET /events`,
  Last-Event-ID replay, `REPLAY_LOG_CAPACITY = 256`). 17 tests.
- Stage 9 — real MQTT client: `RumqttcTransport` wraps
  `rumqttc::AsyncClient` + `EventLoop`; `RumqttcConfig` builder with
  `to_mqtt_options()`; `MqttQos::to_rumqttc()` mapping; publish/
  subscribe/`poll()` event-loop advancement. 25 tests.
- Stage 10 — real GraphQL resolver bridge + axum HTTP/WS integration
  in themql-graphql. `GraphqlResolverBridgeImpl` wraps
  `Arc<dyn themql_core::Resolver>`; `DispatchBridgeImpl` wraps
  `Arc<dyn themql_core::MessageHandler>`. `QueryRoot.resource`/
  `resources`, `MutationRoot.dispatch`, `SubscriptionRoot.subscribe`
  (placeholder stream — real themql-message stream wiring is a
  follow-up). `GraphqlSchemaImpl` holds the built schema.
  `serve_graphql(schema) -> axum::Router` mounts `POST /graphql`
  (query/mutation) + `GET /graphql` (WS subscription). Core
  `Resolver`/`MessageHandler` made dyn-compatible (boxed futures,
  `Send + Sync` supertraits, `ResolverBoxed` blanket-impl adapter
  preserves `async fn` ergonomics). themql-mqtt: removed redundant
  `Send + Sync` bounds on `impl MessageHandler` params. specs/graphql.toml
  gained `http_library = "axum"` + `integration = "async-graphql-axum"`.
  +8 tests in themql-graphql (6 → 13). Workspace 240 → 305 tests.
- Stage 11 — final validation: all 7 gates green
  (`cargo fmt --check`, `cargo clippy --workspace --all-targets
  -- -D warnings`, `cargo test --workspace` — 305 tests, `cargo deny
  check`, `cargo machete --with-metadata`, `scripts/ci_guard.py`,
  `cargo metadata --no-deps`). Commit `828b373` pushed to
  `feat/phase-2-heavy-dep-wiring`, PR #4 open.

### Phase 2 progress (2026-08-20)

- Stage 1 — themql-cache backends (L1 HashMap FIFO, L2 moka, L3 valkey
  stub, TieredCache orchestrator) + themql-storage backend (HelixStorage
  in-memory fallback). +20 tests.
- Stages 2-5 — transport adapters: themql-sse (SseEvent wire format,
  TokioSsePublisher/Stream broadcast-backed, 13 tests), themql-mqtt
  (subject↔topic mapping, JSON codec, 14 tests), themql-graphql
  (#[Object] QueryRoot/MutationRoot, SubscriptionRoot marker only, 6
  tests).
- Stages 6-8 — themql-runtime (TokioRuntime tests, 8 desktop-feature),
  themql-desktop (tokio + ratatui/crossterm TUI, 13 tests),
  themql-embedded (SensorDriver tests, 10 tests).
- Stage 9 — themql-analysis (polars + rayon: PolarsDatasetBuilder,
  RayonAnalysisPipeline, 9 tests), themql-training (tch behind
  `tch-backend` feature: TchTrainer, 5 default tests), themql-inference
  (tch behind `tch-backend` feature: TchInferenceEngine, 6 default
  tests).
- Stage 10 — CacheKey reconciliation (themql-cache re-exports from
  themql-query per spec; removed local def + blake3 dep). Created
  themql-schema crate (20th crate) with canonical shared types matching
  specs/training.toml exactly. Fixed spec deviations in themql-artifact
  (FeatureDType: Bool not I32; NormalizationSpec: simple enum not struct
  variants; FeatureSchema: has normalization field). Updated
  themql-artifact/training/inference to re-export from themql-schema.
  6 new tests in themql-schema.
- Stage 11 — installed cargo-deny/cargo-machete/cargo-bloat. cargo deny
  check passes (added BSL-1.0 + CDLA-Permissive-2.0 licenses). cargo
  machete clean (removed 11 unused deps across 8 crates). cargo bloat
  passes (3.9MiB desktop binary). Wrote opencode.json (permissions +
  validate/safety-gate commands). Wrote scripts/ci_guard.py (TOML parse
  + crate-name invariant).

### Environment constraint (added 2026-08-20)

This environment has 7.8GB RAM, no swap. polars-core OOM-kills rustc
during test compilation unless `CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=none` is set. The `tch-backend`
feature compiles clean but its tests are not run (libtorch download +
RAM). Use `CARGO_BUILD_JOBS=1` to further reduce memory pressure.

### Known gotcha: build environment RAM constraint

This environment has 7.8GB RAM and no swap. polars-core (used by
themql-analysis) OOM-kills rustc during test compilation. To build/test
polars-dependent crates, use `CARGO_BUILD_JOBS=1
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=none`.
The `tch-backend` feature in themql-training/themql-inference compiles
clean but its tests are not run (libtorch download + RAM). BUG-0007
(workspace compile breakage) is RESOLVED — all workspace tests pass.

### Dependency stack (resolved against crates.io 2026-08-19)

- serde 1, serde_json 1
- tokio 1, rayon 1, apalis 0.6
- async-graphql 8.0.0-rc.5
- embassy-executor 0.10, embassy-sync 0.8
- cachelito 0.16 (L1), moka 0.12 (L2), valkey 0.0.0-alpha5 (L3), helix-db 3 (L4/storage)
- dioxus 0.5 (UI), polars 0.55 (analysis)
- tch 0.24 (ML tensors — published as `tch`, not `tch-rs`)
- ndarray 0.17 (raw ML-side array buffers only — NOT for GNC/estimation/inference linear algebra)
- nalgebra 0.35 (conventional numerics: EKF, covariance, quaternion, rotation matrices, linear algebra)
- clap 4 (CLI), ratatui 0.30 (TUI)

New workspace deps added 2026-08-20 (Phase 1 spec deepening):

- `nalgebra = "0.35"` (conventional numerics)
- `uuid = { version = "1", features = ["v7", "serde"] }` (identity)
- `blake3 = "1"` (cache key hashing)
- `thiserror = "2"` (error derives)
- `serde-big-array = "0.5"` (for [f64; 441] covariance serde)

Note: `valkey` is alpha; `cachelito` is a proc-macro for function caching
(may need reassessment for L1 use case in Phase 1).

### Numerical policy (amended 2026-08-20)

nalgebra is the canonical library for conventional numerical
computation in non-ML crates (EKF state/covariance, quaternion
attitude, rotation matrices, linear algebra). tch (published as
`tch`, not `tch-rs`) remains canonical for ML tensors/training/
inference. ndarray is retained only as a workspace dependency for
raw ML-side array buffers; it must not be used for GNC, estimation,
or inference linear-algebra paths. This amends `prompts/SYSTEM.md`,
`prompts/ARCHITECT.md`, `SPEC.toml`, `Cargo.toml`, and `AGENTS.md`.

### Canonical sensors

- NEO-6M GPS
- BME280 barometer
- GY-LSM6DS3 IMU

### theDAF integration

`Metis-Avionics/theDAF` is an integration-only component for legacy data
access, analysis support, and migration reference. theMQL core must not depend
on theDAF. the embedded binary must not depend on theDAF.

## Decision log (chronological)

- 2026-08-20: Phase 3 Stages 1-11 complete — all placeholder/stub
  implementations replaced with real backends. themql-artifact: real
  BLAKE3 HashValidator + FileArtifactLoader + BincodeArtifactWriter.
  themql-storage: SledStorage (disk-backed) + HelixStorage alias +
  ByPredicate query. themql-cache: L1 lru + L2 moka + L3 redis +
  key→subject index + demotion. themql-estimation: real quaternion
  EKF with Jacobian + Joseph-form Kalman gain (`K = PHᵀ(HPHᵀ+R)⁻¹`,
  `P = (I-KH)P(I-KH)ᵀ + KRKᵀ`), SensorModel<M> trait, GpsModel,
  BaroModel, BayesianEstimator. themql-analysis: polars-backed types +
  StorageAnalysisPipeline. themql-training: real TchTrainer (MLP +
  Adam + MSE + TorchScript export) behind `tch-backend`.
  themql-inference: real TchInferenceEngine (CModule load +
  forward_ts + deadline check + rollback) behind `tch-backend`.
  themql-sse: real broadcast + axum serve_sse + Last-Event-ID replay.
  themql-mqtt: RumqttcTransport + RumqttcConfig. themql-graphql:
  GraphqlResolverBridgeImpl + GraphqlSchemaImpl + serve_graphql axum
  integration. Core `Resolver`/`MessageHandler` made dyn-compatible.
  305 tests pass workspace-wide. Commit `828b373`, PR #4.
- 2026-08-20: Phase 3 Stage 10 complete — real GraphQL resolver bridge
  + axum HTTP/WS integration in themql-graphql. Made core
  `Resolver`/`MessageHandler` dyn-compatible by boxing futures (return
  `Pin<Box<dyn Future + Send + '_>>`, add `Send + Sync` supertraits).
  Added `ResolverBoxed` trait + blanket `impl<T: ResolverBoxed>
  Resolver for T` so implementors keep `async fn` ergonomics; the spec
  signature `async fn resolve(&self, query: &Query, ctx: &Context) ->
  Result<Response, Error>` is preserved semantically. The task required
  `Arc<dyn themql_core::Resolver>`; the trait was previously not
  dyn-compatible (used `impl Future`). Zero existing implementors in
  the workspace, so no call-site breakage. 301 tests pass
  workspace-wide.
- 2026-08-20: Phase 2 Stage 11 complete — installed safety-critical
  tooling (cargo-deny, cargo-machete, cargo-bloat). cargo deny check
  passes (added BSL-1.0 + CDLA-Permissive-2.0 licenses). cargo machete
  clean (removed 11 unused deps). cargo bloat passes. Wrote opencode.json
  (permissions + validate/safety-gate commands). Wrote
  scripts/ci_guard.py (TOML parse + crate-name invariant). 240 tests
  pass. Full validation green.
- 2026-08-20: Phase 2 Stage 10 complete — cross-crate type
  reconciliation. CacheKey: themql-cache now re-exports from
  themql-query (per specs/cache.toml). Created themql-schema (20th
  crate) as canonical home for shared schema types (FeatureSchema,
  NormalizationSpec, ModelFormat, FeatureDType, FeatureSpec,
  ValidationMetrics, TrainedModel). User decision: "Move types to a
  new shared crate" to avoid circular dep between themql-training and
  themql-artifact. Fixed spec deviations: FeatureDType has Bool (not
  I32), NormalizationSpec is simple enum (not struct variants),
  FeatureSchema has normalization field. 240 tests pass workspace-wide.
  Full validation green.
- 2026-08-20: Phase 2 Stages 2-5 + 9 complete — wired SSE/MQTT/GraphQL
  transport adapters (SseEvent wire format + broadcast-backed
  publisher/stream; subject↔topic mapping + JSON codec; #[Object]
  QueryRoot/MutationRoot with EmptySubscription in tests). Wired
  polars+rayon into themql-analysis (PolarsDatasetBuilder,
  RayonAnalysisPipeline). Wired tch behind `tch-backend` feature into
  themql-training (TchTrainer) and themql-inference
  (TchInferenceEngine). 234 tests pass workspace-wide (default
  features). Full validation green: cargo fmt --check, cargo clippy
  --workspace --all-targets -- -D warnings, cargo test --workspace.
  BUG-0007 resolved. The `tch-backend` feature compiles clean but tests
  not run (libtorch + RAM). Environment requires
  CARGO_PROFILE_DEV_DEBUG=0 to build polars test artifacts.
- 2026-08-20: Phase 2 Stage 1 complete — wired real cache tier backends
  (L1 HashMap FIFO, L2 moka, L3 valkey stub, TieredCache orchestrator)
  and real storage backend (HelixStorage in-memory fallback). 59 tests
  across the two crates.
- 2026-08-20: Phase 2 Stages 6-8 complete — runtime impls + desktop
  binary + embedded binary wired with real deps (tokio, ratatui/
  crossterm). 29 tests across the three crates.
- 2026-08-20: Phase 1 complete — spec deepening + nalgebra amendment +
  all 19 crates implemented. Deepened ALL 19 specs from thin/medium to deep
  with concrete types/traits/signatures. Implemented real Rust source for all
  19 crates (traits + types + error types + unit tests). Selection = filter +
  projection combined (user decision, core.toml). Runtime = TWO separate traits
  DesktopRuntime (tokio) + EmbeddedRuntime (embassy), NO shared trait (user
  decision, runtime.toml). EstimatorState = 21-dim (user decision,
  state_estimation.toml). 184 tests pass (183 unit + 1 doc). Full validation
  green: cargo fmt --all --check, cargo check --workspace, cargo clippy
  --workspace --all-targets -- -D warnings, cargo test --workspace, TOML
  parse, cargo metadata. New workspace deps: nalgebra, uuid, blake3, thiserror,
  serde-big-array. Known limitations: EKF uses v0.1 simplified identity-gain
  (placeholder); artifact HashValidator uses placeholder fold hash (NOT real
  SHA-256); inference has no tch-backed impl yet; EmbassyRuntime sleep is a
  stub; heavy deps (tch, polars, dioxus, ratatui, embassy) deferred in
  training/analysis/desktop/embedded; CacheKey defined locally in
  themql-cache, reconcile with themql-query later.
- 2026-08-20: nalgebra amendment — nalgebra 0.35 replaces ndarray for
  non-ML numerics (EKF, covariance, quaternion, rotation, linear algebra).
  ndarray demoted to raw ML-side buffers only. Updated SYSTEM.md,
  ARCHITECT.md, SPEC.toml, Cargo.toml, AGENTS.md, MEMORY.md.
- 2026-08-19: v0.1 spec drop. Split `themql-ai` into `themql-training` (desktop)
  + `themql-inference` (embedded). README rewritten as pointer (v0.0 inline
  spec retained only in git history). Prompts split into six role files.
- 2026-08-19: Adopted living-docs rule. After every turn, update README,
  SECURITY, SESSION, HANDOVER, CHANGELOG, BUGS, MEMORY, AGENTS_SYNC.
- 2026-08-19: Created `AGENTS.md` — OpenCode convention file. Compact
  restatement of authority hierarchy, validation commands, architectural
  constraints, dependency policy, workspace layout, forbidden patterns,
  final report format. Points at `prompts/SYSTEM.md` as the full contract.
- 2026-08-19: Completed subsystem spec set. Wrote 5 new deep specs
  (core.toml, storage.toml, graphql.toml, mqtt.toml, sse.toml). Rewrote
  message.toml + query.toml as subordinate to core.toml (canonical type
  definitions moved to core.toml). Trimmed transport.toml to bridges +
  cross-cutting constraints. Expanded model_artifact.toml with new
  `themql-artifact` crate (19 total). Resolved all workspace dep versions
  against crates.io. Wired up 6 crate Cargo.tomls with real deps.
  `cargo check --workspace` passes for all 19 crates.
- 2026-08-19: Toolchain hardening. Updated `rust-toolchain.toml` with
  components (rustfmt, clippy, rust-src, miri) and embedded targets
  (thumbv7em, riscv32imc). Created `.cargo/config.toml` (mold + sccache
  configs, opt-in via comments; cargo aliases). Created `deny.toml`
  (cargo-deny: licenses, advisories, bans, sources — bans pyo3/cpython/
  rusqlite). Created `TETANUS.md` — NASA JPL Power of Ten rules adapted
  for Rust, with enforcement mechanisms and safety-critical crate
  identification. Updated `AGENTS.md` with safety-critical validation
  commands (cargo deny, machete, bloat, miri) and TETANUS summary.
  Added `[profile.dev]`, `[profile.release]`, `[profile.bench]` to root
  Cargo.toml.

### Validation baseline

At end of 2026-08-20 (Phase 3 Stages 1-11 complete):

- 44+ TOML files parse via `python3 tomllib` + `scripts/ci_guard.py`.
- `cargo metadata --no-deps --format-version 1` resolves.
- `cargo fmt --all --check` — clean.
- `cargo check --workspace` — passes for all 20 crates.
- `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings.
- `cargo test --workspace` — 305 tests pass (304 unit + 1 doc, default features).
- `cargo deny check` — passes (advisories ok, bans ok, licenses ok,
  sources ok; 10 RUSTSEC advisory ignores for unmaintained/unsound
  transitive deps).
- `cargo machete --with-metadata` — clean (no unused deps).
- `cargo bloat --release --crates -p themql-desktop` — passes (3.9MiB).
- `scripts/ci_guard.py` — all checks pass.
- The `tch-backend` feature in themql-training/themql-inference compiles
  clean; tests not run (libtorch + RAM constraints).

### Phase 2 status: Stages 1-12 COMPLETE

All 20 crates have real `src/` content (traits + types + error types +
unit tests) + real backends/impls wired behind the trait surfaces
(cache tiers, storage, transport adapters, runtime, desktop/embedded
binaries, analysis/training/inference heavy deps). 240 tests pass
workspace-wide. Safety-critical tooling installed and passing
(cargo-deny, cargo-machete, cargo-bloat). opencode.json + CI guard
script in place. Phase 3 then replaced all remaining placeholders with
real backends (see Phase 3 progress above). Follow-ups (not blockers):

- Real nonlinear quaternion EKF dynamics + Jacobian-based Kalman gain
  — DONE (Phase 3 Stage 4: real Jacobian + Joseph-form covariance
  update in themql-estimation).
- Real BLAKE3 in themql-artifact HashValidator — DONE (Phase 3 Stage
  1: replaced the placeholder fold hash with `blake3::hash`).
- Real training loop in TchTrainer + real model loading/forward pass
  in TchInferenceEngine — DONE (Phase 3 Stages 6/7, behind
  `tch-backend`). Run `tch-backend` feature tests once a beefier
  environment is available (>7.8GB RAM).
- Real analysis pipelines in RayonAnalysisPipeline — partially done
  (polars DataFrame path is real; storage-query path uses byte-length
  proxy values).
- Real L1/L2/L3/L4 cache backends — DONE (Phase 3 Stage 3: L1 lru,
  L2 moka, L3 redis, L4 sled via themql-storage, key→subject index
  for pattern invalidation). L3 needs a running redis-server for live
  operation (degrades to `TierUnavailable` when absent).
- Real network I/O for transport adapters — DONE for SSE (axum
  server) and MQTT (rumqttc client); GraphQL has a real resolver
  bridge + axum HTTP/WS but `SubscriptionRoot.subscribe` emits a
  placeholder stream (real themql-message stream wiring is a
  follow-up).
- Real `themql-desktop` serve/analyze/train/validate/telemetry
  subcommands — follow-up (currently print banners only).
- Real `themql-embedded` embassy main — follow-up (stub on x86 host;
  embassy requires thumbv7em target).
- Install mold + sccache for faster builds.
- 2026-08-20: `themql-sse` real broadcast + axum server live.
  `futures-util` is now a workspace dependency (used by themql-sse for
  `stream::unfold`/`Stream`). `serve_sse` returns an `axum::Router`
  with `GET /events` honoring `Last-Event-ID`; replay log cap
  `REPLAY_LOG_CAPACITY = 256` per subject.
