# CHANGELOG.md

Chronological record of changes to theMQL. Newest entries at the top.
Update after every turn (see `MEMORY.md` standing rules).

## [Unreleased]

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
