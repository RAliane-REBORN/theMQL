# CHANGELOG.md

Chronological record of changes to theMQL. Newest entries at the top.
Update after every turn (see `MEMORY.md` standing rules).

## [Unreleased]

### 2026-08-20 — Phase 4: MQTT bridge + auth + embedded embassy main

Four steps completing the Phase 4 follow-ups:

#### Step 1 — Fix machete + merge PR #5
- Removed unused `serde_json` dev-dep from `themql-analysis` (flagged
  by CI `cargo machete` job on PR #5).
- All 8 CI jobs green. PR #5 merged to main (`983e529`).

#### Step 2 — Wire MQTT-to-SSE bridge into serve subcommand
- Added `themql-mqtt` dependency to `themql-desktop`.
- `MqttToSseBridge` implements `MessageHandler`: incoming MQTT
  messages are decoded and re-published to the SSE publisher via
  `SsePublisher::broadcast()`. Routes via `themql-core Message`,
  never adapter-to-adapter (per `specs/transport.toml [bridges]`).
- New CLI args: `--enable-mqtt`, `--mqtt-host`, `--mqtt-port`,
  `--mqtt-client-id`. Background tokio task drives rumqttc event loop.
- Added `serve_sse_with_publisher` to `themql-sse`: accepts
  `Arc<TokioSsePublisher>` so the bridge and HTTP handler share the
  same publisher instance.
- +3 tests (15 total in themql-desktop).

#### Step 3 — Auth via better-auth + MQTT credentials
- Created `specs/auth.toml`: authn (GraphQL sessions via better-auth,
  MQTT username/password), authz (role-based ACLs: admin/operator/
  observer), constraints (no custom auth engine, no secrets in source).
- Added `better-auth` 0.10 (features: axum, rustls) to workspace deps.
- `RumqttcConfig` gains `username`/`password` fields +
  `with_credentials()` builder → `MqttOptions::set_credentials()`.
- `--enable-auth` + `--auth-secret` (or `THEMQL_AUTH_SECRET` env var)
  builds `BetterAuth` with `MemoryDatabaseAdapter` +
  `EmailPasswordPlugin`, mounts auth routes on the axum router.
- `--mqtt-username` / `--mqtt-password` for broker authn.
- +5 tests (3 MQTT credentials, 2 desktop auth).

#### Step 4 — Real embassy embedded main
- `#![no_std]` + `#![no_main]` via `cfg_attr(target_os = "none")`.
  Host stub retained for `cargo check/test --workspace` on x86.
- Embassy executor with `platform-cortex-m` + `executor-thread`.
- 7 embassy tasks per `specs/embedded.toml [tasks]`:
  gps (10Hz), baro (50Hz), imu (200Hz), estimator (200Hz drain),
  telemetry (10Hz), inference (5Hz), command (idle).
- Inter-task channels: `embassy_sync::channel::Channel` with
  `CriticalSectionRawMutex`, capacity 8.
- `HeapString`: fixed-capacity (64B) string for no_std error messages.
- `TaggedReading`: sensor reading tagged with kind for the estimator.
- Task config constants matching spec (GPS_RATE_HZ, IMU_RATE_HZ, etc.).
- Panic handler with spin_loop. `try_spawn` helper avoids unwrap.
- Cross-compiles clean: `cargo check --target thumbv7em-none-eabihf`.
- New CI job: `embedded-check` (thumbv7em cross-compile).
- +7 tests (16 total in themql-embedded).

**Totals**: 325 tests pass workspace-wide (was 306). 21 specs (was 20).

Four workstreams completing the Phase 3 follow-ups:

#### Workstream A — living docs refresh + PR #4 merge
- Synced all 8 living docs to Phase 3's actual end state (removed stale
  placeholder claims about EKF, HashValidator, cache backends).
- Closed BUG-0008 (sled temp test now stable after Stage 2).
- Committed `81af681`, pushed, merged PR #4 to main (`4fee9d9`).
- Created `feat/phase-3-followups` branch off main.

#### Workstream B — real GraphQL subscriptions
- Added `GraphqlSubscriptionSource` trait (dyn-compatible) +
  `JsonValueStream` type alias.
- Implemented `GraphqlSubscriptionSource` for `TokioSsePublisher`:
  `subscribe_stream` calls `SsePublisher::add_subscriber`, wraps the
  `SseStream` in `SseStreamAdapter` (background task + mpsc channel
  converts `SseEvent` → `serde_json::Value`).
- `SubscriptionRoot::with_source(source)` configures a live event
  source. `subscribe(subject)` now returns a real `Stream` of
  `Json<Value>` events from the SSE publisher.
- `SubscriptionRoot::default()` returns an error on subscribe (no
  source configured).
- +2 tests: `subscription_with_source_streams_real_events` (broadcast
  → receive via GraphQL subscription), `subscription_without_source_
  returns_error`.
- themql-graphql: 13 → 15 tests. Added `themql-sse` + `tokio` (sync)
  deps. Removed unused `serde` dep.

#### Workstream D — GitHub Actions CI
- Created `.github/workflows/ci.yml` with 8 jobs: `fmt`, `check`,
  `clippy`, `test`, `toml-sanity`, `ci-guard`, `deny`, `machete`.
- Uses `dtolnay/rust-toolchain@stable` + `Swatinem/rust-cache@v2`.
- `tch-backend` feature explicitly skipped (libtorch too heavy for
  free runners — documented in workflow comments).
- Runs on push to `feat/*` branches + PRs to `main`.

#### Workstream C — real themql-desktop subcommands
- Rewrote all 5 subcommand bodies to compose real crate APIs:
  - `serve`: builds `GraphqlSchemaImpl` (real resolver bridge + dispatch
    bridge + subscription source from `TokioSsePublisher`) + `serve_sse`
    axum router, merges routers, binds TCP, runs with graceful shutdown
    (Ctrl-C via `tokio::signal`).
  - `analyze`: reads JSON data file, builds `PolarsDatasetBuilder`, runs
    `RayonAnalysisPipeline`, prints row/column/null stats.
  - `train`: behind `tch-backend` feature — loads bincode dataset,
    builds `TrainingConfig`, runs `TchTrainer::train`, packages via
    `BincodeArtifactWriter::write` + `write_to_file`. Without feature:
    returns error with rebuild instructions.
  - `validate`: loads `ModelArtifact` via `FileArtifactLoader`, prints
    format/schema_version/model_bytes/hash.
  - `telemetry`: opens `SledStorage`, queries by `SubjectPattern`,
    prints entries.
- Added `tch-backend` feature to themql-desktop Cargo.toml (optional
  dep on `themql-training`). Added deps: `themql-graphql`,
  `themql-sse`, `themql-analysis`, `themql-artifact`, `themql-storage`,
  `themql-schema`, `axum`, `serde_json`.
- 12 tests (was 13 — removed TUI test that requires a terminal; added
  error-path tests for train/validate/telemetry).
- Removed unused deps: `serde_json` from themql-analysis main deps
  (moved to dev-deps), `tempfile` from themql-desktop dev-deps.

#### Validation
- 306 tests pass workspace-wide (default features).
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo deny check`, `cargo machete --with-metadata`,
  `scripts/ci_guard.py` — all green.

### 2026-08-20 — Phase 3 Stages 1-11 complete: all placeholders replaced with real backends

Phase 3 replaced every remaining placeholder/stub implementation with
real backends across all 20 crates. Commit `828b373`, PR #4. 305 tests
pass workspace-wide (304 unit + 1 doc, default features). Full
validation green (all 7 gates).

#### Stage 1 — themql-artifact (real BLAKE3 + file/bincode I/O)
- `HashValidator` now uses `blake3::hash` for the 32-byte integrity
  digest (placeholder fold hash replaced).
- `FileArtifactLoader` — loads a `ModelArtifact` from a filesystem
  path (bincode deserialisation).
- `BincodeArtifactWriter` — writes a `TrainedModel` to a
  `ModelArtifact` on disk (computes blake3 hash, bincode serialisation).

#### Stage 2 — themql-storage (real sled disk backend)
- `SledStorage` — disk-backed sled key-value store implementing
  `Storage`. `open(path)`, `open_temp()`, `get`/`put`/`delete`/
  `query(ByKey | BySubjectPattern | ByPredicate)`.
- `HelixStorage` is now `pub type HelixStorage = SledStorage`
  (backward-compatible alias; the in-memory HashMap fallback is gone).
- BUG-0008 (flaky `helix_alias_works`) resolved — sled temp open is
  stable once the crate is fully wired.

#### Stage 3 — themql-cache (real L1 lru + L2 moka + L3 redis + index)
- `L1Cache` — backed by `lru::LruCache` (was `HashMap` + FIFO).
- `L2Cache` — backed by `moka::sync::Cache` (unchanged from Phase 2,
  confirmed real).
- `L3Cache` — backed by `redis::Client` (was `valkey` alpha stub
  returning `TierUnavailable` on every op; now real redis with
  `get`/`set`/`del`/`expire`).
- Key→subject index added to L1/L2 for pattern invalidation
  (was effectively a no-op).
- Demotion on hit: `get` promotes entries to higher tiers.

#### Stage 4 — themql-estimation (real Jacobian EKF)
- `Ekf` now uses real Jacobian-based Kalman gain with Joseph-form
  covariance update: `K = PHᵀ(HPHᵀ+R)⁻¹`,
  `P = (I-KH)P(I-KH)ᵀ + KRKᵀ` (was simplified identity-gain).
- `SensorModel<M>` trait + `GpsModel` (7-dim: pos+vel+clock_bias) +
  `BaroModel` (2-dim: altitude+bias) with real `predict_measurement`,
  `jacobian`, `innovation`.
- `BayesianEstimator` trait + `propagate_uncertainty` + `sample`.

#### Stage 5 — themql-analysis (polars + storage query)
- `StorageAnalysisPipeline` — queries `themql-storage` and builds
  `PolarsAnalysisResult` from the results.
- Polars-backed `Dataset` type (re-exported by themql-training).

#### Stage 6 — themql-training (real TchTrainer)
- `TchTrainer::train` builds an MLP, trains with Adam + MSE for
  `config.epochs` (optional early stopping), exports to TorchScript
  bytes via `CModule::create_by_tracing`. Behind `tch-backend` feature.

#### Stage 7 — themql-inference (real TchInferenceEngine)
- `TchInferenceEngine::load()` runs `HashValidator::validate`,
  rejects on errors, deserialises into `tch::CModule`.
  `infer()` runs `forward_ts`, checks `inference_deadline_ms`,
  populates `confidence` from output norm.
  `rollback()` reloads previous bytes. Behind `tch-backend` feature.

#### Stage 8 — themql-sse (real axum server)
- `TokioSsePublisher` (broadcast channel + bounded event log),
  `TokioSseStream` (broadcast receiver → SSE event stream),
  `serve_sse` (axum router with `GET /events`, Last-Event-ID replay).

#### Stage 9 — themql-mqtt (real rumqttc client)
- `RumqttcTransport` wraps `rumqttc::AsyncClient` + `EventLoop`.
  `RumqttcConfig` builder with `to_mqtt_options()`.
  `MqttQos::to_rumqttc()` mapping. publish/subscribe/`poll()`.

#### Stage 10 — themql-graphql (real resolver bridge + axum HTTP/WS)
- `GraphqlResolverBridgeImpl` wraps `Arc<dyn themql_core::Resolver>`.
  `DispatchBridgeImpl` wraps `Arc<dyn themql_core::MessageHandler>`.
  `QueryRoot`/`MutationRoot` real fields.
  `SubscriptionRoot.subscribe` emits placeholder stream (follow-up).
  `serve_graphql(schema) -> axum::Router` (POST /graphql + GET /graphql WS).
  Core `Resolver`/`MessageHandler` made dyn-compatible (boxed futures,
  `ResolverBoxed` blanket-impl adapter).

#### Stage 11 — final validation
- All 7 gates green. Commit `828b373` (36 files, +4799/-811).
- `deny.toml` updated: 10 RUSTSEC advisory ignores for unmaintained/
  unsound transitive deps; removed `rusqlite` ban (no longer relevant).
- 6 spec files amended (model_artifact, storage, cache, mqtt, sse,
  graphql).

### 2026-08-20 — Phase 3 Stages 6 & 7: real training loop + real inference forward pass

Replaced the placeholder `tch-backend` implementations in
`themql-training` and `themql-inference` with real training and
inference logic. Both are behind the `tch-backend` feature; default
features still expose the trait surface only.

#### themql-training (Stage 6)
- `TchTrainer::train` now runs a real feed-forward training loop:
  builds an MLP (`feat_dim` → `2×feat_dim` hidden → `label_dim`) via
  `tch::nn::seq` + `tch::nn::linear`, trains with Adam
  (`tch::nn::Optimizer`, `config.weight_decay`) and MSE loss for
  `config.epochs`, batches by `config.batch_size`.
- Optional early stopping: tracks best loss, stops after
  `EarlyStoppingConfig.patience` stale epochs.
- Exports the trained network to `TorchScript` bytes via
  `CModule::create_by_tracing` + `CModule::save` to a temp file (read
  back as bytes). Returns a `TrainedModel` (format `TorchScript`).
- Validates config (`epochs`/`batch_size`/`learning_rate` > 0) and
  dataset (non-empty, schema width matches frame width, label rows
  match feature rows); returns `TrainingError::Diverged` on NaN/Inf
  loss.
- `Dataset` re-exports the polars-backed
  `themql_analysis::Dataset` (was `Vec<Vec<f64>>`). Added `polars` +
  `themql-analysis` deps.
- New tests (default): `dataset_reexports_polars_dataset_type`.
- New tch-backend tests: `tch_trainer_rejects_zero_epochs`,
  `tch_trainer_artifact_writer_accepts_model` (round-trips the trained
  model through `BincodeArtifactWriter::write`).

#### themql-inference (Stage 7)
- `TchInferenceEngine::load()` now runs
  `themql_artifact::HashValidator::new().validate(&artifact)` and
  rejects on any `report.errors`; rejects empty bytes on EVERY load
  (not just the first, as the placeholder did); deserialises
  `model_bytes` into a `tch::CModule` via `CModule::load_data` and
  stores it pre-allocated at activation.
- `infer()` runs the real forward pass
  (`module.forward_ts(&[input])` under `no_grad`), extracts the output
  to a 21-dim `state_correction` via `f_copy_data::<f32>`, checks
  `budget.inference_deadline_ms` against actual latency
  (`DeadlineExceeded` on overrun), populates `confidence` from the
  output norm (`1/(1+||correction||)`).
- `rollback()` reloads the previous bytes from the `RollbackHandle`
  into a fresh `CModule` and restores it as active.
- Re-exported `ArtifactValidator` from `themql-artifact` so the
  validator trait is in scope for `load()`.

#### Pre-existing cleanup (required by new deps)
- `themql-analysis`: fixed pre-existing clippy lints (doc backticks,
  unused `StorageValue` import, `cast_precision_loss` via a
  `usize_to_f64` helper, `manual_async_fn` allow on
  `StorageAnalysisPipeline`).
- `themql-artifact`: fixed pre-existing clippy lints (redundant
  closures in `FileArtifactLoader`, `similar_names` in tests).

#### Validation
- `cargo test -p themql-training`: 6 tests, 0 failures (default).
- `cargo test -p themql-inference`: 6 tests, 0 failures (default).
- `cargo check -p themql-training --features tch-backend`: clean.
- `cargo check -p themql-inference --features tch-backend`: clean.
- `cargo clippy -p themql-training -p themql-inference -p themql-analysis
  -p themql-artifact --all-targets -- -D warnings`: clean.
- `cargo fmt --check` clean for all touched crates.
- `cargo check --workspace`: green.
- TOML parse sanity passes.
- `tch-backend` feature tests NOT run (libtorch C++ build needs more
  RAM than this environment has — 7.8GB, no swap).
- All code `#![forbid(unsafe_code)]`.

### 2026-08-20 — Phase 3 Stage 10: real GraphQL resolver bridge + axum HTTP/WS integration

Replaced the placeholder `themql-graphql` roots with a real resolver
bridge, concrete root fields, a built-schema holder, and an axum HTTP/WS
integration. Made the core `Resolver` and `MessageHandler` traits
dyn-compatible so the bridge can hold `Arc<dyn Resolver>` / `Arc<dyn
MessageHandler>` as required.

#### themql-graphql
- `GraphqlResolverBridge` is now dyn-compatible (boxed future).
- `GraphqlResolverBridgeImpl` wraps `Arc<dyn themql_core::Resolver>`,
  builds a `Query` from the GraphQL field/args, delegates to the
  resolver, projects `ResponseValue` → JSON.
- `DispatchBridge` (new dyn-compatible trait) + `DispatchBridgeImpl`
  wrap `Arc<dyn themql_core::MessageHandler>` for mutations.
- `QueryRoot` now has `resource(subject, selection, args)` and
  `resources(pattern)` fields calling the resolver bridge.
- `MutationRoot` now has `dispatch(subject, payload)` calling the
  dispatch bridge.
- `SubscriptionRoot` now has a `#[Subscription] subscribe(subject)`
  field emitting a placeholder stream (TODO: real `themql-message`
  stream wiring in a later stage).
- `GraphqlSchemaImpl` (new) holds a built
  `async_graphql::Schema<QueryRoot, MutationRoot, SubscriptionRoot>`.
- `serve_graphql(schema) -> axum::Router` mounts `POST /graphql`
  (query/mutation via `GraphQL`) and `GET /graphql` (subscription via
  WebSocket upgrade, `GraphQLSubscription`).
- New deps: `axum`, `async-graphql-axum`, `futures-util`, `serde` (was
  already a transitive need).
- 13 tests (was 5): kept existing error-mapping tests, added
  resolver-bridge construction, schema build with real bridge,
  `resource` query integration, `dispatch` mutation integration,
  `serve_graphql` router build, `response_value_to_json` unit tests,
  invalid-subject rejection.
- `#![forbid(unsafe_code)]`, `#![deny(warnings)]`, clippy pedantic clean.

#### themql-core
- `Resolver` and `MessageHandler` made dyn-compatible: `resolve`/`handle`
  now return `Pin<Box<dyn Future<...> + Send + '_>>` and the traits have
  `Send + Sync` supertraits.
- Added `ResolverBoxed` trait + blanket `impl<T: ResolverBoxed>
  Resolver for T` so implementors keep the `async fn` ergonomics. The
  spec signature `async fn resolve(&self, query: &Query, ctx:
  &Context) -> Result<Response, Error>` is preserved semantically.
- Added `use std::pin::Pin;`.
- Zero existing implementors in the workspace, so no call-site breakage.

#### themql-mqtt
- Removed now-redundant `Send + Sync` bounds on `impl MessageHandler`
  parameters (clippy `implied_bounds_in_impls` after the
  `MessageHandler` supertrait change).

#### specs/graphql.toml
- `[implementation]` gained `http_library = "axum"` and `integration =
  "async-graphql-axum"`.

Validation: `cargo test -p themql-graphql` (13 tests), `cargo clippy -p
themql-graphql --all-targets -- -D warnings` clean, `cargo fmt` clean
for touched crates. Workspace `cargo test --workspace` green (301
tests). No `unsafe`. TOML parse sanity passes.

### 2026-08-20 — Phase 2 Stage 12: final validation + commit + PR

Ran the complete validation suite (all 7 gates green), updated all 8
living docs to mark Stages 1-12 complete, committed all Phase 2 work
as `d97fcca` (48 files, +6441/-960), pushed to
`feat/phase-2-heavy-dep-wiring`, created PR #4.

- `cargo fmt --check` — PASS
- `cargo clippy --workspace --all-targets -- -D warnings` — PASS
- `cargo test --workspace` — PASS (240 tests, 0 failures)
- `cargo deny check` — PASS
- `cargo machete --with-metadata` — PASS
- `scripts/ci_guard.py` — PASS
- `cargo metadata --no-deps` — PASS
- `tch-backend` feature `cargo check` — PASS (tests not run, RAM)

### 2026-08-20 — Phase 2 Stage 11: safety-critical tooling + opencode.json + CI guard

Installed and ran the safety-critical validation tooling per
`AGENTS.md` + `TETANUS.md`. Wrote `opencode.json` and CI guard script.

#### Stage 11a — safety-critical tooling

- Installed `cargo-deny`, `cargo-machete`, `cargo-bloat` via
  `cargo install`.
- `cargo deny check` — passes. Added `BSL-1.0` (Boost Software License)
  and `CDLA-Permissive-2.0` to `deny.toml` allowed licenses (both
  OSI-approved, compatible with MIT).
- `cargo machete --with-metadata` — clean. Removed 11 unused deps
  across 8 crates: `serde` from themql-message, `valkey` from
  themql-cache, `themql-message` from themql-sse, `themql-core` from
  themql-embedded, `serde_json` from themql-analysis, `themql-query`
  from themql-graphql, `embassy-executor` + `embassy-sync` +
  `themql-message` from themql-mqtt, `themql-message` from themql-query,
  `rayon` from themql-training, `helix-db` from themql-storage,
  `blake3` from themql-core.
- `cargo bloat --release --crates -p themql-desktop` — passes. Binary
  is 3.9MiB, .text section 939KiB (23.8%), no unusual bloat.
- Removed conflicting `deny`/`machete` cargo aliases from
  `.cargo/config.toml` (they shadowed the external subcommands).

#### Stage 11b — opencode.json

- `opencode.json` — project config with `$schema`,
  `instructions: ["AGENTS.md"]`, permission rules (cargo/python3/git/gh
  allowed, `rm` asks), and two custom commands: `validate` (full
  validation suite) + `safety-gate` (TETANUS gate for safety-critical
  crates).

#### Stage 11c — CI guard script

- `scripts/ci_guard.py` — checks: (1) all TOML files parse, (2) crate
  names in Cargo.toml match directory names, (3) spec crate names
  match workspace members. All checks pass.

### 2026-08-20 — Phase 2 Stage 10: cross-crate type reconciliation

Reconciled duplicated types across crates. Created `themql-schema` (20th
crate) as the canonical home for shared schema types. Fixed spec
deviations in `themql-artifact`'s enum variants. Full workspace
validation passes: 240 tests, clippy clean, fmt clean.

#### Stage 10a — CacheKey reconciliation

- `crates/themql-cache/Cargo.toml` — added `themql-query` dep, removed
  `blake3` dep (no longer used locally).
- `crates/themql-cache/src/lib.rs` — removed local `CacheKey` definition
  (tuple struct + `from_bytes()`/`hash_of()`/`as_bytes()`), now
  re-exports `CacheKey` from `themql-query` per `specs/cache.toml`.
- `crates/themql-query/src/lib.rs` — added `Serialize`/`Deserialize`
  derives + `hash_of()` method to `CacheKey` (was missing). Added
  `serde` import.

#### Stage 10b — themql-schema crate (20th crate)

- `crates/themql-schema/Cargo.toml` — new crate, deps: `serde`,
  dev-deps: `serde_json`.
- `crates/themql-schema/src/lib.rs` — canonical shared types matching
  `specs/training.toml` exactly: `ModelFormat`, `FeatureDType` (F32/F64/
  I64/Bool — fixed from themql-artifact's F32/F64/I32/I64),
  `FeatureSpec`, `FeatureSchema` (has `normalization` field — fixed from
  themql-artifact which omitted it), `NormalizationSpec` (simple enum
  None/Standard/MinMax/Custom — fixed from themql-artifact's struct
  variants), `ValidationMetrics`, `TrainedModel`. 6 tests pass.
- `specs/schema.toml` — new spec for the schema crate.
- `Cargo.toml` — added `themql-schema` to workspace members + deps.
- `specs/training.toml` — added `specs/schema.toml` to references,
  added `implementation_note` to `[api.FeatureSchema]`.
- `specs/model_artifact.toml` — updated re-export comments from
  "training.toml" to "themql-schema".

#### Stage 10c — updated dependent crates

- `crates/themql-artifact/Cargo.toml` — added `themql-schema` dep.
- `crates/themql-artifact/src/lib.rs` — removed local definitions of
  `ModelFormat`, `FeatureDType`, `FeatureSpec`, `FeatureSchema`,
  `NormalizationSpec`, `ValidationMetrics`, `TrainedModel`; now
  re-exports from `themql-schema`. Updated tests for new `FeatureSchema`
  shape (with `normalization` field).
- `crates/themql-training/Cargo.toml` — added `themql-schema` dep.
- `crates/themql-training/src/lib.rs` — removed local definitions of
  `FeatureDType`, `FeatureSpec`, `NormalizationSpec`, `FeatureSchema`,
  `ModelFormat`, `ValidationMetrics`, `TrainedModel`; now re-exports
  from `themql-schema`. Moved `BTreeMap` import to `tch-backend` cfg
  block.
- `crates/themql-inference/Cargo.toml` — added `themql-schema` dep.
- `crates/themql-inference/src/lib.rs` — updated re-exports: `TrainedModel`
  now from `themql-schema`, `ActivationHandle`/`ArtifactError`/
  `ModelArtifact` still from `themql-artifact`. Updated tch-backend test
  imports.

### 2026-08-20 — Phase 2 Stages 2-5 + 9: transport adapters + analysis/training/inference heavy-dep wiring

Completed the remaining Phase 2 heavy-dep wiring: SSE/MQTT/GraphQL
transport adapters (Stages 2-5) and polars+rayon/tch wiring into
analysis/training/inference (Stage 9). Full workspace validation now
passes: `cargo fmt --check`, `cargo clippy --workspace --all-targets
-- -D warnings`, `cargo test --workspace` (234 tests, default
features). BUG-0007 (workspace compile breakage) resolved.

#### Stage 2 — themql-sse transport adapter

- `crates/themql-sse/Cargo.toml` — added `serde_json` to deps, `tokio`
  with `rt`/`sync`/`io-util` features to deps, `tokio` with `macros`/
  `rt-multi-thread` to dev-deps.
- `crates/themql-sse/src/lib.rs` — added `SseEvent::to_wire_string()`
  (SSE wire format: `id:`/`event:`/`data:` lines), `SseEvent::from_message()`
  (Message→SseEvent projection), `TokioSsePublisher` (broadcast
  channel-backed), `TokioSseStream` (broadcast receiver → SSE event
  stream). 13 tests pass.

#### Stage 3 — themql-mqtt transport adapter

- `crates/themql-mqtt/Cargo.toml` — moved `serde_json` from dev-dep to
  main deps.
- `crates/themql-mqtt/src/lib.rs` — added `subject_to_topic()` (identity
  mapping per spec), `topic_to_subject()`, `encode_message()` (Message→
  JSON bytes), `decode_message()` (JSON bytes→Message). 14 tests pass.

#### Stage 4-5 — themql-graphql transport adapter

- `crates/themql-graphql/Cargo.toml` — added `tokio` with `macros` to
  dev-deps.
- `crates/themql-graphql/src/lib.rs` — added `#[Object]` impls for
  `QueryRoot` (placeholder fields) and `MutationRoot` (placeholder
  fields). `SubscriptionRoot` kept as marker only (no `#[Object]` —
  `SubscriptionType` trait not implemented). Tests use
  `async_graphql::EmptySubscription`. 6 tests pass.

#### Stage 9 — themql-analysis (polars + rayon)

- `crates/themql-analysis/Cargo.toml` — added `polars` and `rayon` to
  deps.
- `crates/themql-analysis/src/lib.rs` — added `PolarsAnalysisResult`
  (wraps `polars::frame::DataFrame` + `AnalysisStats`),
  `PolarsDatasetBuilder` (constructs `DataFrame` from headers + rows),
  `RayonAnalysisPipeline` (parallel null-count via `par_iter`). 9 tests
  pass.

#### Stage 9 — themql-training (tch behind `tch-backend` feature)

- `crates/themql-training/Cargo.toml` — added `tch` (optional,
  `download-libtorch` feature) and `rayon` to deps; added
  `[features] tch-backend = ["dep:tch"]`.
- `crates/themql-training/src/lib.rs` — added `TchTrainer` implementing
  `Trainer` behind `cfg(feature = "tch-backend")`: placeholder training
  loop that creates tensors, runs `tanh`, and produces model bytes. 5
  tests pass (default features); `tch-backend` tests compile clean but
  not run (libtorch download + RAM constraints).

#### Stage 9 — themql-inference (tch behind `tch-backend` feature)

- `crates/themql-inference/Cargo.toml` — added `tch` (optional,
  `download-libtorch` feature); added `[features] tch-backend =
  ["dep:tch"]`.
- `crates/themql-inference/src/lib.rs` — added `TchInferenceEngine`
  implementing `InferenceEngine` behind `cfg(feature = "tch-backend")`:
  loads/activates `ModelArtifact`, runs dummy forward pass (tensor
  `tanh`), rollback support. 6 tests pass (default features);
  `tch-backend` tests compile clean but not run (libtorch + RAM).

### 2026-08-20 — Phase 2 Stage 1: themql-cache backends + themql-storage backend wired

Wired real cache tier backends into `themql-cache` and a real storage
backend into `themql-storage`. Both crates compile, test, and clippy-clean
under `-D warnings`.

#### Stage 1 — themql-cache backends

- `crates/themql-cache/Cargo.toml` — added `moka` (features `sync`),
  `valkey`, `tokio` (features `rt`, `sync`), and `serde_json` to
  dependencies; `tokio` (features `macros`, `rt-multi-thread`) to
  dev-dependencies. Added `themql-storage` workspace dep (L4 reference).
- `crates/themql-cache/src/lib.rs` — added three tier backends plus the
  `TieredCache` orchestrator implementing the `Cache` trait:
  - `L1Cache` — bounded `HashMap<CacheKey, CacheEntry>` + `VecDeque`
    FIFO eviction, guarded by `RwLock`/`Mutex`. The `cachelito` crate
    (v0.16) is a procedural-macro memoisation library backed by
    process-wide `&'static Lazy` singletons keyed on `String`, which is
    incompatible with the per-instance, `CacheKey`-keyed L1 the spec
    requires; per the spec (`custom_cache_engine = false`) and the task
    brief, L1 is the spec-sanctioned `HashMap` + capacity fallback.
  - `L2Cache` — wraps `moka::sync::Cache<CacheKey, CacheEntry>` (sync).
  - `L3Cache` — wraps the `valkey` 0.0.0-alpha5 driver. The driver is
    synchronous, `&str`-keyed, and exposes only `set`/`get` (no `DEL`,
    no `EXPIRE`, no binary values), so it cannot faithfully store a
    `CacheEntry`. Construction succeeds (URL stored); every operation
    returns `CacheError::TierUnavailable(L3)` until the driver matures
    (task-brief-sanctioned stub).
  - `TieredCache<S: Storage>` — holds optional L1/L2/L3 + L4 `Storage`;
    `get()` walks L1→L2→L3→L4 promoting on hit; `put()` writes through
    to all enabled tiers; `invalidate()` removes from all tiers;
    `invalidate_pattern()` scans L1/L2 keys (best-effort, no
    key→subject index yet so effectively no-op).
- Added 14 new tests (L1 round-trip + capacity eviction + invalidate +
  invalidate_all; L2 round-trip + invalidate; L3 construction +
  tier-unavailable; tiered L1 hit, L2-hit-promotes-to-L1, put
  write-through, invalidate-removes-from-all, L4 miss, disabled-policy
  short-circuit). Existing 16 tests unchanged. Total: 30 passing.

#### Stage 1 — themql-storage backend

- `crates/themql-storage/Cargo.toml` — added `tokio` (features
  `macros`, `rt-multi-thread`) to dev-dependencies.
- `crates/themql-storage/src/lib.rs` — added `HelixStorage` implementing
  the `Storage` trait. The `helix-db` v3.0 crate is an async HTTP client
  for a running Helix instance over `/v2/query` (graph-traversal DSL,
  not a key-value store, requires a live server). Per the task brief,
  `HelixStorage` is an in-memory `HashMap<StorageKey, StorageValue>`
  fallback behind a `Mutex` now; the real helix-db adapter is
  FOLLOW-UP_REQUIRED. `query` supports `ByKey` and `BySubjectPattern`
  (best-effort subject reconstruction from the opaque key string);
  `ByPredicate` returns `StorageError::QueryError`.
- Added 6 new tests (put/get round-trip, delete, not-found-returns-none,
  query-by-key, query-by-subject-pattern, query-by-predicate-returns-
  error). Existing 23 tests unchanged. Total: 29 passing.

#### Validation

- `cargo fmt -p themql-cache -p themql-storage --check` — clean.
- `cargo clippy -p themql-cache -p themql-storage --all-targets -- -D
  warnings` — clean.
- `cargo test -p themql-cache -p themql-storage` — 30 + 29 = 59 pass.

#### Notes / follow-up

- L1 cachelito integration is deferred: the v0.16 API is macro/`'static`
  singleton-based and does not fit per-instance `CacheKey`-keyed L1.
- L3 valkey integration is deferred: the 0.0.0-alpha5 driver lacks
  `DEL`/`EXPIRE`/binary values; operations stubbed to
  `TierUnavailable`.
- L4 helix-db integration is deferred: the v3.0 client requires a live
  HTTP server and is graph-DSL-oriented, not key-value; `HelixStorage`
  is in-memory.
- `invalidate_pattern` against L1/L2 is effectively a no-op until a
  key→subject index is added (`CacheKey` is an opaque BLAKE3 hash with
  no reversible mapping to the originating subject).

### 2026-08-20 — Phase 2 Stages 6-8: runtime impls + desktop binary + embedded binary wired

Wired real runtime implementations and the two binary crates with real
dependencies (tokio, ratatui/crossterm). All three crates compile, test,
and clippy-clean under both feature flags.

#### Stage 6 — themql-runtime

- `crates/themql-runtime/Cargo.toml` — added `[dev-dependencies]` tokio
  with `rt`, `rt-multi-thread`, `time`, `sync`, `test-util`, `macros`
  so the desktop-feature tests can construct a multi-thread runtime.
- `crates/themql-runtime/src/lib.rs` — added 8 real `TokioRuntime`
  tests behind `cfg(feature = "desktop")` in a `tokio_tests` submodule:
  `spawn_returns_value`, `sleep_waits_at_least_requested_duration`,
  `timeout_fires_when_future_is_too_slow`,
  `timeout_succeeds_when_future_completes_in_time`,
  `channel_send_and_recv_roundtrip`,
  `cancellation_token_cancel_then_observed`,
  `cancellation_token_cancelled_resolves_if_already_cancelled`,
  `spawn_join_error_when_task_panics`. The existing 6 error-type tests
  remain. Verified `cargo check -p themql-runtime --features embedded`
  compiles (embassy sleep stub returns `std::future::pending`); did not
  run embassy tests on x86 (no embassy executor on host).
- Clippy clean on both `--features desktop` and `--features embedded`
  with `-D warnings`.

#### Stage 7 — themql-desktop (binary)

- `crates/themql-desktop/Cargo.toml` — added tokio (`rt`, `rt-multi-thread`,
  `macros`, `net`, `io-util`) and ratatui (workspace = true). ratatui 0.30
  re-exports crossterm via its default `crossterm` feature, so no separate
  `crossterm` crate dep was needed.
- `crates/themql-desktop/src/main.rs` — rewrote the entry point:
  - `#[tokio::main] async fn main() -> Result<(), Box<dyn std::error::Error>>`.
  - clap CLI dispatch (`Serve`/`Analyze`/`Train`/`Validate`/`Telemetry`/
    `Tui`) prints a per-command banner; `Tui` calls `tui_main()`.
  - `tui_main()` uses `ratatui::init()` / `ratatui::restore()` and polls
    crossterm key events at 100 ms; quits on `q` or `Esc`.
  - `draw_dashboard()` renders a 3-pane vertical layout
    (telemetry stream / state estimate + covariance / controller state +
    actuator commands) with bordered, bold-titled blocks per
    `specs/desktop.toml [api.TuiLayout]`.
  - 13 unit tests: CLI parse for each subcommand, help-text contains
    expected strings, dispatch succeeds for each command, and a
    `TestBackend` test that `draw_dashboard` renders without panicking.

#### Stage 8 — themql-embedded (binary)

- `crates/themql-embedded/src/main.rs` — the existing stub already
  provided `SensorKind`, `SensorError` (5 variants: `BusError`,
  `Timeout`, `SensorNotResponding`, `InvalidReading`,
  `CalibrationRequired`), `SensorDriver` trait, `GpsDriver`,
  `BaroDriver`, `ImuDriver`. Updated the main stub banner to
  "themql-embedded: stub — embassy runtime requires thumbv7em target".
  Added 6 more tests: `gps_driver_construction`,
  `baro_driver_construction`, `imu_driver_construction`,
  `sensor_error_all_variants_distinct`,
  `sensor_driver_trait_object_dispatch`, `sensor_error_is_std_error`.
  No embassy added (won't compile on x86 host, per spec). 10 tests pass.

#### Validation (this turn)

- `cargo fmt -p themql-runtime -p themql-desktop -p themql-embedded --check`
  — clean.
- `cargo clippy -p themql-runtime -p themql-desktop -p themql-embedded
  --all-targets -- -D warnings` — zero warnings.
- `cargo clippy -p themql-runtime --features desktop -- -D warnings`
  — zero warnings.
- `cargo clippy -p themql-runtime --features embedded -- -D warnings`
  — zero warnings.
- `cargo test -p themql-runtime -p themql-desktop -p themql-embedded`
  — 6 + 13 + 10 = 29 tests pass (default features).
- `cargo test -p themql-runtime --features desktop` — 14 tests pass
  (6 default + 8 desktop-feature).
- NOTE: `cargo check --workspace` currently fails in `themql-cache` and
  `themql-storage` due to **pre-existing uncommitted changes** in those
  crates that were in the working tree before this turn began (confirmed
  via `git stash`). Those are out of scope for Stages 6-8 and not
  introduced by this turn.

#### Stubs

- `themql-embedded` main is a stub (`embassy runtime requires thumbv7em
  target`). The real `#[embassy_executor::main]` is a future task per
  `specs/embedded.toml [entry]`.
- `themql-desktop` dispatch prints banners; real serve/analyze/train/
  validate/telemetry implementations are future tasks. The TUI is real
  (renders + handles quit keys) but panes are empty placeholders pending
  telemetry wiring.

### 2026-08-20 — Phase 1 complete: spec deepening + nalgebra amendment + all 19 crates implemented

Phase 1 is done. Every crate now has real `src/` content (traits + types +
error types + unit tests). No 0-line stubs remain.

#### Stage 0 — nalgebra amendment

- `prompts/SYSTEM.md` — dependency policy + new "Numerical policy" section.
- `prompts/ARCHITECT.md` — Numerical policy section rewritten: nalgebra for
  EKF/covariance/quaternion/linear-algebra, tch for ML, ndarray demoted to
  raw ML-side buffers only.
- `SPEC.toml` — added `numerics = "nalgebra"` to `[dependencies]`.
- `Cargo.toml` — added `nalgebra = "0.35"` to workspace.dependencies.
- `AGENTS.md` — dependency policy + note.
- `MEMORY.md` — dependency stack + Numerical policy subsection.

#### Stage 1 — spec deepening (all 19 specs)

Deepened ALL 19 specs from thin/medium to deep with concrete types/traits/
signatures:

- `specs/core.toml` — Subject/SubjectSegment/SubjectPattern/PatternSegment,
  Selection (filter + projection combined, per user decision), Projection,
  Predicate, Comparison, FieldPath, CachePolicy, CacheTier, InvalidationHint,
  Cancellation, Payload, FormatTag, Metadata, Timestamp, Deadline,
  ResponseValue, etc.
- `specs/message.toml` — Subject grammar (ABNF), Subject API, Serializer
  trait, MessageError.
- `specs/query.toml` — QueryExecutor trait, CacheKeyer trait, CacheKey,
  Batcher, QueryError.
- `specs/runtime.toml` — TWO separate traits (DesktopRuntime tokio,
  EmbeddedRuntime embassy), NO shared trait (per user decision).
- `specs/transport.toml` — Bridge trait, BridgeRoute, TransportKind,
  TransportError.
- `specs/cache.toml` — Cache trait, CacheEntry, CacheHit, CacheError, tier
  semantics.
- `specs/telemetry.toml` — TelemetryMessage, SensorReading enum,
  per-sensor structs (GpsReading+GpsFix, BarometerReading, ImuReading),
  StateEstimateReading, CovarianceReading (21x21 = [f64;441]),
  ControllerStateReading, ModelStateReading, DiagnosticsReading,
  TelemetryError.
- `specs/mqtt.toml` — SubscriptionId, MqttQos.
- `specs/graphql.toml` — QueryRoot/MutationRoot/SubscriptionRoot bodies.
- `specs/sse.toml` — SseEvent fields.
- `specs/storage.toml` — StorageKey/StorageValue/StorageQuery/StorageResultSet
  Rust types pinned.
- `specs/gnc.toml` — GncState (21-dim, nalgebra UnitQuaternion), Controller
  trait, PidGains/PidController, LqriMatrices/LqriController,
  HybridSwitchingPolicy/HybridController, ActuatorCommand, GncError, TETANUS
  section.
- `specs/state_estimation.toml` — EstimatorState (21-dim:
  pos[3]+vel[3]+quat[4]+angvel[3]+accelbias[3]+gyrobias[3]+barobias[1]+
  gpsbias[1] = 21, per user decision), Estimator trait, Ekf struct,
  SensorModel trait, EstimationError, TETANUS section.
- `specs/inference.toml` — InferenceEngine trait, ResourceBudget,
  RollbackHandle, InferenceInput/Output, InferenceError, TETANUS section.
- `specs/training.toml` — Trainer trait, Dataset, TrainingConfig,
  PruningConfig/Strategy, SparsificationConfig/Strategy, TrainedModel,
  TrainingError.
- `specs/model_artifact.toml` — ModelArtifact, ArtifactMetadata,
  CompatibilityInfo, TensorSchema, ValidationReport, RuntimeInfo,
  ActivationHandle, PruningMetadata.
- `specs/analysis.toml` — AnalysisPipeline trait, AnalysisInput/Result/
  Stats, DatasetBuilder, ThedafAdapter, AnalysisError.
- `specs/desktop.toml` — Cli (clap), Command enum
  (Serve/Analyze/Train/Validate/Telemetry/Tui), TuiLayout, DesktopRoot,
  main entry.
- `specs/embedded.toml` — SensorDriver trait, SensorError, embassy task
  topology, main entry, TETANUS section.

#### Stage 2 — all 19 crates implemented

| Crate | Tests | Notes |
|---|---|---|
| themql-core | 31 + 1 doc | Message, Query, Response, Error, Context, Subject, Selection, Predicate, CachePolicy, Cancellation, Timestamp, Deadline, Metadata, Payload, Operation, Resource, Resolver/MessageHandler/QueryExecutor traits |
| themql-message | 4 | Serializer trait, JsonSerializer, MessageError |
| themql-query | 11 | CacheKey, CacheKeyer, DefaultCacheKeyer, QueryError, Batcher |
| themql-runtime | 6 | DesktopRuntime + EmbeddedRuntime traits, TokioRuntime (desktop feature), EmbassyRuntime (embedded feature), RuntimeError |
| themql-cache | 16 | Cache trait, CacheEntry, CacheHit, CacheError |
| themql-storage | 23 | Storage/Reader/Writer traits, StorageKey/Value/Query/ResultSet, StorageError |
| themql-transport | 14 | Bridge trait, BridgeRoute, TransportKind, SubjectRewrite, TransportError |
| themql-telemetry | 14 | TelemetryMessage, all sensor structs, TelemetryError |
| themql-mqtt | 9 | MqttTransport/Publisher/Subscriber traits, MqttQos, SubscriptionId, MqttError |
| themql-graphql | 4 | GraphqlSchema/GraphqlResolverBridge traits, Query/Mutation/Subscription roots, GraphqlError |
| themql-sse | 8 | SseStream/SsePublisher traits, SseEvent, SseError |
| themql-artifact | 5 | ArtifactValidator/Loader/Writer traits, ModelArtifact, ArtifactMetadata, ValidationReport, ArtifactError, TrainedModel. TETANUS-compliant |
| themql-gnc | 7 | GncState, Controller trait, PidController, LqriController, HybridController, ActuatorCommand, GncError. TETANUS-compliant |
| themql-estimation | 6 | Estimator trait, Ekf, EstimatorState 21-dim, EstimationError. TETANUS-compliant. v0.1 simplified EKF |
| themql-inference | 6 | InferenceEngine trait, ResourceBudget, RollbackHandle, InferenceInput/Output, InferenceError. TETANUS-compliant |
| themql-training | 5 | Trainer trait, Dataset, TrainingConfig, TrainedModel, TrainingError |
| themql-analysis | 5 | AnalysisPipeline trait, AnalysisInput/Result, DatasetBuilder, ThedafAdapter, AnalysisError |
| themql-desktop | 5 | Cli clap struct, Command enum, main entry stub |
| themql-embedded | 4 | SensorDriver trait, SensorError, main stub. TETANUS-compliant |

Total: 183 unit tests + 1 doc test = 184 tests, all green.

#### New workspace deps added this turn

- `nalgebra = "0.35"` (conventional numerics)
- `uuid = { version = "1", features = ["v7", "serde"] }` (identity)
- `blake3 = "1"` (cache key hashing)
- `thiserror = "2"` (error derives)
- `serde-big-array = "0.5"` (for [f64; 441] covariance serde)

#### Validation (all green)

- `cargo fmt --all --check` — clean.
- `cargo check --workspace` — passes for all 19 crates.
- `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings.
- `cargo test --workspace` — 183 unit + 1 doc = 184 tests, all pass.
- `python3 tomllib` parse — all TOML files OK.
- `cargo metadata --no-deps` — resolves.

#### Known limitations / follow-ups

- `themql-estimation` EKF uses v0.1 simplified identity-gain updates, NOT
  full nonlinear quaternion dynamics + Jacobian-based Kalman gain.
- `themql-artifact` HashValidator uses a placeholder fold hash, NOT real
  SHA-256. Must replace before deployment.
- `themql-inference` has no concrete tch-backed impl yet (libtorch build dep).
- `themql-runtime` EmbassyRuntime sleep is a stub; embassy 0.10 Spawner is
  not Send/Sync so EmbeddedRuntime trait was relaxed from spec.
- `themql-training/analysis/desktop/embedded` defer heavy deps (tch, polars,
  dioxus, ratatui, embassy) — traits + minimal types only.
- CacheKey defined locally in themql-cache; reconcile with themql-query
  CacheKey later.
- cargo-deny/machete/bloat/sccache/mold still not installed.

### 2026-08-20 — Phase 1: themql-artifact, themql-gnc, themql-estimation, themql-inference

Implemented the four safety-critical crates per their specs
(`specs/model_artifact.toml`, `specs/gnc.toml`,
`specs/state_estimation.toml`, `specs/inference.toml`) and TETANUS.md.
All four carry `#![forbid(unsafe_code)]`, `#![deny(warnings)]`,
`#![warn(clippy::pedantic)]`, `#![warn(clippy::unwrap_used,
clippy::expect_used, clippy::panic)]`. nalgebra 0.35 is used for all
non-ML numerics.

#### Added — themql-artifact (`crates/themql-artifact`)

- `ModelFormat` — `TorchScript | SafeTensors | Onnx` (snake_case serde).
- `FeatureSchema` + `FeatureSpec` + `FeatureDType` — canonical input
  schema types re-exported by training/inference.
- `NormalizationSpec` — `None | Standard{mean,std} | MaxAbs{max_abs}`
  (tagged serde).
- `ValidationMetrics` — loss + accuracy + custom BTreeMap.
- `PruningMetadata` — strategy + target/achieved sparsity.
- `TensorSchema` — input/output shapes + dtype string.
- `CompatibilityInfo` — runtime/architecture version + tensor schema.
- `ArtifactMetadata` — full metadata bundle.
- `TrainedModel` — canonical producer-side type (model_bytes + format +
  feature_schema + validation_metrics).
- `ModelArtifact` — the desktop→embedded transfer unit (model_bytes +
  format + 32-byte hash + metadata + compatibility + schema_version).
- `ValidationReport` — hash_ok/schema_ok/compatibility_ok/integrity_ok
  + errors vec.
- `RuntimeInfo` — target runtime description for compatibility checks.
- `ActivationHandle` — model_id + activated_at Timestamp.
- `ArtifactError` — 9 variants (thiserror + serde); `From<ArtifactError>
  for themql_core::Error` (internal_error).
- `ArtifactValidator`/`ArtifactLoader`/`ArtifactWriter` traits per
  `specs/model_artifact.toml [api]`.
- `HashValidator` — reference validator with placeholder fold hash
  (real SHA-256 is a future task; avoids a crypto dep in this
  safety-critical crate for v0.1).
- 5 unit tests (construction JSON round-trip, error display, hash
  mismatch detection, compatibility pass/reject).
- `Cargo.toml`: themql-core, thiserror, serde_json (kept serde + [lib]).

#### Added — themql-gnc (`crates/themql-gnc`)

- `GncState` — 21-dim state using nalgebra fixed-size types
  (Vector3, UnitQuaternion); `new()` returns zero-state.
- `Setpoint` — target position/velocity/attitude/angular_velocity.
- `ActuatorCommand` — fixed `[f64; 8]` actuator vector.
- `ControllerKind` — `Pid | Lqri | Hybrid`.
- `PidGains` + `PidController` — PID with integral windup saturation
  (±MAX_INTEGRAL); `step()` allocation-free.
- `LqriMatrices` + `LqriController` — LQRI with SMatrix 8×21/8×3/
  21×21/8×8; integral action on 3-axis tracking error.
- `HybridSwitchingPolicy` — `AlwaysPid | AlwaysLqri |
  LqriWithPidFallback(PidGains) | PidWithLqriOuter(LqriMatrices) |
  Schedule(fn(GncState) -> ControllerKind)`. Schedule holds a function
  pointer (not closure) per TETANUS rule 9.
- `HybridController` — applies policy then delegates.
- `Controller` trait — `step()` + `kind()`; `Send + Sync`.
- `GncError` — 6 variants (thiserror); `From<GncError> for
  themql_core::Error`.
- 7 unit tests (zero state, PID finite output, LQRI finite output,
  hybrid==pid under AlwaysPid, dt<=0 err, dt<0 err, quaternion norm).
- `Cargo.toml`: themql-core, nalgebra, thiserror, [lib].

#### Added — themql-estimation (`crates/themql-estimation`)

- `STATE_DIM = 21` const.
- `EstimatorState` — `x: SVector<f64,21>`, `P: SMatrix<f64,21,21>`,
  `t: f64`; `new()` zero state + identity covariance.
- `ImuReading`/`GpsReading`/`BarometerReading` — minimal local stubs
  matching telemetry schema (canonical types live in themql-telemetry
  when implemented).
- `Estimator` trait — predict/update_gps/update_baro/state/covariance;
  `Send + Sync`.
- `Ekf` — v0.1 simplified EKF: predict advances time + adds process
  noise; update_gps/update_baro apply simplified identity-gain updates.
  Full nonlinear quaternion dynamics + Jacobian-based Kalman gain is a
  future task. No heap alloc after `new()`.
- `EstimationError` — 6 variants (thiserror); `From<EstimationError>
  for themql_core::Error`.
- 6 unit tests (identity covariance, dt<=0 err, gps update accepts
  valid, state dim 21, cov trace non-negative, predict advances time).
- `Cargo.toml`: themql-core, nalgebra, thiserror, [lib].

#### Added — themql-inference (`crates/themql-inference`)

- Re-exports from themql-artifact: `ModelArtifact`, `ArtifactError`,
  `TrainedModel`, `ActivationHandle`.
- `ResourceBudget` — cpu_budget_pct/memory_budget_bytes/
  inference_deadline_ms/adaptation_deadline_ms; `new()` validates
  cpu <= 100%.
- `RollbackHandle` — previous_model_id + previous_model_bytes;
  `restore()` consumes and returns bytes (rejects empty bytes/id).
- `InferenceInput` — 21-dim nalgebra state vector + ResourceBudget
  (sensor snapshot omitted for v0.1 to avoid telemetry dep).
- `InferenceOutput` — state_correction (SVector<21>) + confidence +
  latency_ns.
- `InferenceEngine` trait — load/infer/rollback/active_model;
  `Send + Sync`.
- `InferenceError` — 7 variants (thiserror); `From<InferenceError>`
  and `From<ArtifactError>` for themql_core::Error.
- `now_timestamp()` helper re-exporting themql_core::Timestamp.
- 6 unit tests (budget valid/over-100, error display, rollback restore
  ok/empty-reject/missing-id-reject).
- `Cargo.toml`: themql-core, themql-artifact, nalgebra, thiserror,
  [lib]. NOTE: `tch` deliberately NOT added (libtorch build dep); a
  tch-backed InferenceEngine impl is a future task.

#### TETANUS compliance notes (all four crates)

- `#![forbid(unsafe_code)]` — no unsafe anywhere.
- `#![deny(warnings)]` + `clippy::pedantic` — zero warnings.
- `clippy::unwrap_used`/`expect_used`/`panic` warned at lib level;
  allowed only inside `#[cfg(test)] mod tests`.
- No recursion; all loops over fixed-size arrays/constants.
- No heap alloc after init in gnc/estimation step() paths (nalgebra
  SMatrix/SVector are stack-allocated); inference allows heap alloc
  only at model activation per spec.
- Functions ≤ 60 lines (verified by review).
- ≥ 2 assertions per public function as `if !invariant { return Err }`.
- `non_snake_case` allowed in gnc/estimation for spec-mandated
  mathematical field names (`K`, `Ki`, `Q`, `R`, `P`, `x`).

### 2026-08-20 — Phase 1: themql-cache, themql-transport, themql-storage

Implemented the three cross-cutting library crates per their specs
(`specs/cache.toml`, `specs/transport.toml`, `specs/storage.toml`).
Traits + types + error types only — no concrete backend wiring yet.

#### Added — themql-cache (`crates/themql-cache`)

- `CacheKey` — 32-byte BLAKE3 content-addressed key (`from_bytes`,
  `hash_of`, `as_bytes`). Defined locally (not re-exported from
  `themql-query`) so this crate takes no hard dep on `themql-query`.
- `CacheEntry` — value + inserted_at + ttl + source_tier + etag.
- `CacheHit` — `Hit { value, source_tier } | Miss`.
- `Cache` trait — async (`impl Future`) `get`/`put`/`invalidate`/
  `invalidate_pattern`, matching `specs/cache.toml [api.Cache]`.
- `CacheError` — `TierUnavailable`/`DeserializationError`/
  `SerializationError`/`InvalidPolicy`/`PatternInvalid`.
- Re-exports from core: `CachePolicy`, `CacheTier`, `InvalidationHint`,
  `SubjectPattern`, `SubjectError`.
- `From<SubjectError> for CacheError`, `From<CacheError> for
  themql_core::Error` per the spec's mapping table.
- 16 unit tests (key hashing, hit/miss, entry construction, error
  variants, error→core mapping, JSON round-trips).
- `Cargo.toml`: `themql-core`, `thiserror`, `blake3`, `serde` +
  `serde_json` (dev).

#### Added — themql-transport (`crates/themql-transport`)

- `TransportKind` — `Mqtt | Graphql | Sse` (snake_case serde).
- `SubjectRewrite` — `Prefix(Subject) | Replace(Subject)`.
- `BridgeRoute` — source/target/source_pattern/target_subject/rewrite.
- `Bridge` trait — async `route(msg, source, targets)` per
  `specs/transport.toml [api.Bridge]`.
- `TransportError` — `SourceClosed`/`TargetPublishFailed`/
  `SubjectRewriteFailed`/`SerializationError`/`LoopbackForbidden`.
- Re-exports from core: `Subject`, `SubjectPattern`; re-exports
  `Serializer` from `themql-message`.
- `From<TransportError> for themql_core::Error` (all variants →
  `transport_error`).
- 14 unit tests (kind serde, route construction, rewrite variants,
  error variants, error→core mapping, JSON round-trips).
- `Cargo.toml`: `themql-core`, `themql-message`, `thiserror`, `serde`
  + `serde_json` (dev).

#### Added — themql-storage (`crates/themql-storage`)

- `StorageKey(String)` — opaque stable-hash string (`new`, `as_str`).
- `StorageValue` — bytes + `FormatTag`.
- `StorageQuery` — `ByKey | BySubjectPattern | ByPredicate(json)`.
- `StorageResultSet` — entries + has_more + cursor (`empty`, `is_empty`).
- `Storage` trait — async `get`/`put`/`delete`/`query` per
  `specs/storage.toml [api.Storage]`.
- `StorageReader` trait — read-only `get`/`query`.
- `StorageWriter` trait — write-only `put`/`delete`.
- `StorageError` — `NotFound`/`AlreadyExists`/`ConnectionFailed`/
  `Timeout`/`SerializationError`/`QueryError`/`InternalError`.
- `From<StorageError> for themql_core::Error` per the spec's mapping
  (NotFound/AlreadyExists/QueryError → resolver_error;
  SerializationError → transport_error; ConnectionFailed/Timeout/
  InternalError → internal_error).
- 23 unit tests (key/value construction + serde, query variants, result
  set pagination, error variants, error→core mapping, JSON round-trips).
- `Cargo.toml`: kept existing `helix-db` + `serde`; added
  `themql-core`, `thiserror`, `serde_json`. `helix-db` compiles but is
  not imported in `lib.rs` — the traits are storage-agnostic.

#### Validation

- `cargo fmt -p themql-cache -p themql-transport -p themql-storage` —
  clean.
- `cargo clippy -p themql-cache -p themql-transport -p themql-storage
  --all-targets -- -D warnings` — passes (zero warnings, pedantic +
  deny(warnings)).
- `cargo test -p themql-cache -p themql-transport -p themql-storage` —
  53/53 pass (16 cache + 14 transport + 23 storage).
- `cargo fmt -p themql-cache -p themql-transport -p themql-storage
  --check` — clean.

#### Notes

- No `unsafe` in any of the three crates (`#![forbid(unsafe_code)]`).
- No comments in code; doc comments on all public items.
- Async traits use `#[allow(async_fn_in_trait)]` + `impl Future`
  returns (matching the `themql_core::Resolver` pattern).
- `helix-db` v3.0.0 compiles but is intentionally unused in
  `themql-storage` — the `Storage` trait is backend-agnostic; the L4
  implementation lands in a later phase.
- `CacheKey` defined locally in `themql-cache` (not re-exported from
  `themql-query`) to avoid a hard dep on `themql-query`, which is not
  yet implemented.

### 2026-08-19 — Toolchain hardening + TETANUS.md

Hardened the build toolchain: rust-toolchain components + embedded targets,
cargo-deny config, mold/sccache build config, and the NASA JPL Power of Ten
rules codified as `TETANUS.md`.

#### Added — build toolchain config

- `rust-toolchain.toml` — expanded with `components` (rustfmt, clippy,
  rust-src, miri) and `targets` (x86_64-unknown-linux-gnu, thumbv7em-none-eabi,
  thumbv7em-none-eabihf, riscv32imc-unknown-none-elf).
- `.cargo/config.toml` — mold linker + sccache wrapper configs (opt-in via
  comments; do not break builds when tools absent). Cargo aliases: `chk`,
  `tst`, `fmtc`, `clip`, `deny`, `machete`.
- `deny.toml` — cargo-deny configuration. License allowlist (MIT, Apache-2.0,
  BSD, ISC, Zlib, CC0, etc.). Bans: pyo3, cpython, python3-sys (Python ABI),
  rusqlite (custom database). Advisory checking via RustSec db. Source
  restriction to crates.io only.
- `TETANUS.md` — NASA JPL Power of Ten rules adapted for Rust. Full
  rationale, enforcement mechanisms, per-rule Rust-specific guidance,
  summary table, and safety-critical crate identification (themql-gnc,
  themql-estimation, themql-inference, themql-artifact).

#### Changed — workspace config

- `Cargo.toml` — added `[profile.dev]` (split-debuginfo), `[profile.release]`
  (lto=thin, codegen-units=1, strip=symbols), `[profile.bench]` (lto=thin,
  codegen-units=1).
- `AGENTS.md` — added safety-critical validation section (cargo deny, machete,
  bloat, miri), TETANUS.md summary (10 rules), updated TOML file count to 40+,
  updated workspace crate count to 19, updated dependency names (tch, helix-db).

#### Validation

- 43/43 TOML files parse.
- `cargo metadata --no-deps` resolves.
- `cargo check --workspace` passes for all 19 crates.
- `cargo deny`, `cargo machete`, `cargo bloat` gates defined but not yet
  executable (tools not installed in this environment; will install + run
  when tools are available).

#### Known limitations

- `mold` not installed in this environment; `.cargo/config.toml` link config
  is commented out. Uncomment after `apt install mold`.
- `sccache` not installed; `rustc-wrapper` line is commented out. Uncomment
  after `cargo install sccache`.
- `cargo-deny`, `cargo-machete`, `cargo-bloat` not installed; config files
  are ready for when they are.

### 2026-08-19 — Subsystem spec set completed + Phase 1 dep wiring

Completed the subsystem spec coverage: every crate now has a matching spec.
Wired up real dependencies in 6 crate Cargo.tomls with versions resolved
against crates.io.

#### Added — new specs (5)

- `specs/core.toml` — canonical type spec: Message, Query, Response, Error,
  Context, Resource. Deep: `[types.*]`, `[api]`, `[error_model]`,
  `[lifecycle]`, `[threading]`. Absorbs canonical definitions from
  message.toml and query.toml (those are now subordinate).
- `specs/storage.toml` — helix-db adapter spec. Deep: `[implementation]`,
  `[api]`, `[error_model]`, `[cache_integration]`, `[lifecycle]`,
  `[threading]`.
- `specs/graphql.toml` — async-graphql projection spec. Deep:
  `[schema_generation]`, `[api]`, `[mapping]`, `[error_model]`,
  `[lifecycle]`, `[threading]`.
- `specs/mqtt.toml` — embassy MQTT transport spec. Deep: `[topics]`,
  `[api]`, `[mapping]`, `[error_model]`, `[lifecycle]`, `[threading]`.
- `specs/sse.toml` — SSE stream projection spec. Deep: `[streaming]`,
  `[api]`, `[mapping]`, `[error_model]`, `[lifecycle]`, `[threading]`.

#### Added — new crate

- `crates/themql-artifact/` — 19th crate. Model artifact validation and
  transfer between `themql-training` (desktop) and `themql-inference`
  (embedded). Cargo.toml wired with `serde` dep. Empty `src/lib.rs` stub.

#### Changed — specs (6)

- `specs/transport.toml` — trimmed. Removed per-transport detail blocks
  (`[mqtt]`, `[graphql]`, `[sse]`); they now live in the subordinate
  sub-specs. Kept `[principle]`, `[bridges]`, `[constraints]`. Added
  `[references] subspecs` pointer.
- `specs/message.toml` — rewritten as subordinate to `specs/core.toml`.
  Removed canonical Message type definition (moved to core.toml). Kept
  `[routing]`, `[serialization]`, `[identity]`, `[constraints]`. Added
  `subordinate_to` and `[references] canonical`.
- `specs/query.toml` — rewritten as subordinate to `specs/core.toml`.
  Removed canonical Query model (moved to core.toml) and transport
  projection blocks (moved to sub-specs). Kept `[resolution]`, `[cache]`,
  `[constraints]`. Added `subordinate_to` and `[references] canonical`.
- `specs/model_artifact.toml` — expanded to deep. Added
  `crate = "themql-artifact"`. New sections: `[api]`, `[validation_pipeline]`,
  `[error_model]`, `[lifecycle]`, `[threading]`, expanded `[constraints]`.
- `specs/training.toml` — `[artifact]` section now references
  `crate = "themql-artifact"`.
- `specs/inference.toml` — `[model_artifact]` section now references
  `crate = "themql-artifact"`.

#### Changed — workspace config

- `Cargo.toml` (root) — added `themql-artifact` to `[workspace] members`
  (19 total). Added `themql-artifact` path dep to `[workspace.dependencies]`.
  Resolved all workspace dep versions against crates.io:
  - `tch-rs` → `tch` (actual crate name)
  - `helixdb` → `helix-db` (actual crate name)
  - `async-graphql` → `8.0.0-rc.5`
  - `embassy-executor` → `0.10`, `embassy-sync` → `0.8`
  - `valkey` → `0.0.0-alpha5` (alpha)
  - `cachelito` → `0.16`, `moka` → `0.12`, `helix-db` → `3`
  - `polars` → `0.55`, `tch` → `0.24`, `ndarray` → `0.17`
  - `clap` → `4`, `ratatui` → `0.30`
- `SPEC.toml` — added `artifact = "themql-artifact"` to `[workspace.crates]`
  (19 total).

#### Changed — crate Cargo.tomls wired with real deps (7)

- `crates/themql-core/Cargo.toml` — `serde`, `serde_json`.
- `crates/themql-storage/Cargo.toml` — `helix-db`, `serde`.
- `crates/themql-graphql/Cargo.toml` — `async-graphql`, `themql-core`,
  `themql-query`.
- `crates/themql-mqtt/Cargo.toml` — `embassy-executor`, `embassy-sync`,
  `themql-core`, `themql-message`.
- `crates/themql-sse/Cargo.toml` — `tokio`, `themql-core`, `themql-message`.
- `crates/themql-artifact/Cargo.toml` — `serde`.

All wired crates have explicit `[lib]` sections with `name` and `path`.

#### Validation

- 41/41 TOML files parse via `python3 tomllib`.
- `cargo metadata --no-deps --format-version 1` resolves with 19 members.
- `cargo check --workspace` passes for all 19 crates with real deps
  resolved and downloaded from crates.io.

#### Known limitations carried forward

- `valkey` is `0.0.0-alpha5` (alpha) — may need replacement if unstable.
- `cachelito` is a proc-macro for function caching — may not match the L1
  use case; reassess in Phase 1.
- No real Rust source beyond 0-line stubs — Phase 1 proper.

### 2026-08-19 — AGENTS.md

- Created `AGENTS.md` at repo root — OpenCode convention file. Compact
  restatement of: authority hierarchy, read-these-first prompts, living-docs
  rule, validation commands (cargo fmt/check/test/clippy + TOML parse +
  cargo metadata), non-obvious architectural constraints (the 9 things an
  agent would likely get wrong), dependency policy, workspace layout, repo
  state, forbidden patterns, final report format. Points at
  `prompts/SYSTEM.md` as the full coding-agent contract.

### 2026-08-19 — Living docs adopted

- Adopted the living-docs rule: after every turn, update README, SECURITY,
  SESSION, HANDOVER, CHANGELOG, BUGS, MEMORY, AGENTS_SYNC at the repo root.
- Created all 7 living docs with current state.

### 2026-08-19 — v0.1 specification drop

First real specification + workspace skeleton drop. Replaces the v0.0 inline
spec that previously lived in the README (retained only in git history).

#### Added — specifications

- `SPEC.toml` — v0.1 constitution.
- `specs/message.toml`, `query.toml`, `cache.toml`, `transport.toml`,
  `runtime.toml`, `telemetry.toml`, `state_estimation.toml`, `gnc.toml`,
  `model_artifact.toml`, `analysis.toml`, `desktop.toml`, `embedded.toml`.
- `specs/training.toml` (desktop model creation) and `specs/inference.toml`
  (embedded model execution) — split from the proposed single `ml.toml`.

#### Added — prompts

- `prompts/SYSTEM.md` — authority hierarchy, core architectural rule,
  dependency policy, semantic ownership.
- `prompts/ARCHITECT.md` — desktop/embedded architecture, GNC authority,
  state representation, numerical policy, theDAF policy, ML lifecycle, online
  adaptation, sensors, cache, concurrency.
- `prompts/IMPLEMENTER.md` — implementation procedure, forbidden patterns,
  validation, final report template.
- `prompts/REVIEWER.md` — non-adversarial review checklist.
- `prompts/RED_TEAM.md` — adversarial review, red flags, finding format.
- `prompts/TEST_ENGINEER.md` — testing policy derived from `SPEC.toml
  [quality]` / `[safety]` plus ML-specific tests.

#### Added — workspace skeleton

- `Cargo.toml` — workspace root, 18 members, `[workspace.package]`,
  `[workspace.dependencies]` with placeholder versions, self-referencing
  path deps for all 18 crates.
- `rust-toolchain.toml` — stable channel, no pin.
- `crates/themql-*` × 18 — each with a `Cargo.toml` stub referencing
  `workspace = true` for version/edition/license.
  - 16 library crates with empty `src/lib.rs`.
  - 2 binary crates (`themql-desktop`, `themql-embedded`) with
    `src/main.rs` containing `fn main() {}`.
- `Cargo.lock` generated by `cargo check`.

#### Changed

- `README.md` rewritten as a short pointer. The previous 785-line inline
  v0.0 spec is retained only in git history.

#### Architectural decision: training/inference split

The proposed single `themql-ai` crate and `specs/ml.toml` were replaced
before writing by splitting model creation from model execution:

- `themql-training` (desktop) — tch-rs training, fine tuning, pruning,
  sparsification, artifact generation.
- `themql-inference` (embedded) — tch-rs inference, PINN inference, sparse
  inference, Bayesian update, constrained online adaptation.
- `specs/model_artifact.toml` governs the validated transfer between the two.

`SPEC.toml [ml]` keeps a single top-level key with `[ml.training]`,
`[ml.inference]`, `[ml.authority]`, and `[ml.artifact]` sub-tables.

#### Validation

- 35/35 TOML files parse via `python3 tomllib`.
- `cargo metadata --no-deps --format-version 1` resolves.
- `cargo check --workspace` passes for all 18 crates.

#### Not in this drop

- No real Rust source beyond the two 1-line binary stubs — Phase 1.
- No CI guard script.
- No dependency versions resolved against crates.io — placeholders only.
- No `[profile]` overrides.
## 2026-08-20 — Phase 1: themql-training, themql-analysis, themql-desktop, themql-embedded trait+type stubs

### themql-training (crates/themql-training) — desktop, NOT safety-critical

- New `src/lib.rs`: `TrainerKind` enum (Dense, Pinn, GradientBoosting,
  FineTuning); `FeatureSchema`/`FeatureSpec`/`FeatureDType`/
  `NormalizationSpec` (defined locally — `themql-artifact` lib.rs is
  still empty); minimal `Dataset` (Vec<Vec<f64>> stand-in for polars);
  `TrainingConfig` + `EarlyStoppingConfig` + `PruningConfig` +
  `PruningStrategy` + `SparsificationConfig` + `SparsificationStrategy`
  (Quantization{bits} / Distillation); `TrainedModel` + `ModelFormat` +
  `ValidationMetrics`; `Trainer` async trait; `TrainingError` thiserror
  enum with `From<TrainingError> for themql_core::Error`.
- `Cargo.toml`: added `[lib]` + themql-core, thiserror, serde, serde_json.
- 5 unit tests, all passing.
- Deferred deps: `tch`, `polars` (per task brief; real training impl is
  a future task).

### themql-analysis (crates/themql-analysis) — desktop, NOT safety-critical

- New `src/lib.rs`: `AnalysisInput` enum (Telemetry{from,to,
  subject_pattern} / DataFrame(Vec<Vec<f64>>)); `AnalysisStats`;
  `AnalysisResult`; minimal local `Dataset`; `AnalysisPipeline` async
  trait; `DatasetBuilder` + `build()`; `ThedafAdapter` async trait;
  `AnalysisError` thiserror enum with `From<AnalysisError> for
  themql_core::Error`.
- `Cargo.toml`: added `[lib]` + themql-core, thiserror, serde, serde_json.
- 5 unit tests, all passing.
- Deferred deps: `polars`, `rayon` (per task brief).

### themql-desktop (crates/themql-desktop) — binary, NOT safety-critical

- New `src/main.rs`: clap-derive `Cli` + `Command` enum (Serve, Analyze,
  Train, Validate, Telemetry, Tui) + `ServeArgs`/`AnalyzeArgs`/
  `TrainArgs`/`ValidateArgs`/`TelemetryArgs`; minimal `dispatch()` +
  `main()` returning `ExitCode`. No async runtime.
- `Cargo.toml`: added themql-core, clap with `derive` feature (workspace
  declares `clap = "4"` without features — feature must be added at the
  consumer site).
- 5 unit tests, all passing.
- Deferred deps: `dioxus`, `ratatui`, `tokio` (per task brief).

### themql-embedded (crates/themql-embedded) — binary, TETANUS applies

- New `src/main.rs`: `SensorKind`, `SensorReading` (fixed 32-byte raw
  buffer — no dynamic alloc), `SensorError` (manual Display — keeps
  binary dep-free), `SensorDriver` trait (sync `read` — canonical form
  is async under embassy, which does not compile on the x86 host),
  stub `GpsDriver`/`BaroDriver`/`ImuDriver` impls; stub `main()`
  printing "themql-embedded: stub". Task topology documented in the
  `main` doc-comment.
- `Cargo.toml`: added themql-core only.
- 4 unit tests, all passing.
- Deferred deps: `embassy-executor` (does not compile for x86 host).
- TETANUS: `#![forbid(unsafe_code)]`, no unwrap/expect in non-test code,
  no recursion, fixed bounds, `main` ≤ 60 lines, `clippy::pedantic` +
  `-D warnings` clean.

### Validation (all four crates)

- `cargo fmt -p <each> --check` — clean.
- `cargo clippy -p <each> --all-targets -- -D warnings` — clean.
- `cargo test -p <each>` — 19 tests, all passing.

### Notes

- The 4 target crates compile + lint + test cleanly in isolation.
- The wider workspace still has pre-existing uncommitted breakage in
  `themql-mqtt` (missing `#[derive(Error)]` on `MqttError`, etc.) that
  predates this task and was not touched.
- `themql-artifact`, `themql-telemetry`, `themql-gnc`, `themql-estimation`
  still have empty `lib.rs` files; the four crates implemented here
  define their dependent types locally rather than depending on those
  empty crates, per the task brief.

## 2026-08-20 — Phase 3 Stage 8: themql-sse real broadcast + axum server

- `crates/themql-sse/src/lib.rs`: replaced placeholder `broadcast::Sender<()>`
  with `broadcast::Sender<Arc<SseEvent>>`; `TokioSsePublisher::broadcast`
  now sends real events and maintains a bounded (`VecDeque`, cap 256)
  per-subject event log for Last-Event-ID replay (`replay_after`).
- `TokioSseStream::next_event` returns the real `SseEvent` from the
  channel (was a synthetic `{}`).
- Added `serve_sse(publisher, subject) -> axum::Router` exposing
  `GET /events` producing `text/event-stream` responses with
  `Last-Event-ID` header replay, keep-alive, via `futures_util::stream::unfold`.
- Wire-format helpers (`to_wire_string`, `from_message`, `from_data`)
  unchanged.
- Tests: 17 pass (was 11); added real-event broadcast, multi-subscriber
  broadcast, Last-Event-ID replay, unknown-id full-log replay.
- `#![forbid(unsafe_code)]`, `#![deny(warnings)]`, `clippy::pedantic`
  clean; no inline comments.
- Added `axum.workspace = true` and `futures-util = "0.3"` (workspace
  dep) to `crates/themql-sse/Cargo.toml`; updated tokio features to
  `["rt","sync","net"]`.
- `specs/sse.toml`: added `http_library = "axum"` to `[implementation]`.
