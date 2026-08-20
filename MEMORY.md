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
- Status: greenfield (Phase 1 implemented; 19 crates with real source)
- Language: Rust
- License: MIT
- Repository: https://github.com/Metis-Avionics/theMQL
- Toolchain: stable channel, no version pin
- Workspace resolver: 2
- Branch: `feat/phase-1-spec-deepening` (stacked on
  `feat/v0.1-spec-and-workspace-skeleton`, PR #1 open)

### Crates (19)

17 library crates + 2 binary crates. ALL 19 now have real `src/` content
(traits + types + error types + unit tests). No 0-line stubs remain.

| Crate | Domain | Role | Tests |
|---|---|---|---|
| themql-core | core | semantic owner — canonical types (Message, Query, Response, Error, Context, Resource) + traits | 31 + 1 doc |
| themql-message | core | Serializer trait + JsonSerializer + MessageError | 4 |
| themql-query | core | CacheKey, CacheKeyer, QueryExecutor, Batcher, QueryError | 11 |
| themql-runtime | runtime | DesktopRuntime (tokio) + EmbeddedRuntime (embassy), separate traits | 6 |
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
| themql-embedded | binary | SensorDriver trait, embassy task topology (TETANUS) | 4 |
| themql-desktop | binary | Cli (clap), Command enum, main entry | 5 |

Total: 183 unit tests + 1 doc test = 184 tests, all green.

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

At end of 2026-08-20 (Phase 1 complete — all 19 crates implemented):

- 43+ TOML files parse via `python3 tomllib`.
- `cargo metadata --no-deps --format-version 1` resolves.
- `cargo fmt --all --check` — clean.
- `cargo check --workspace` — passes for all 19 crates.
- `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings.
- `cargo test --workspace` — 183 unit tests + 1 doc test = 184 tests, all pass.
- `cargo deny`, `cargo machete`, `cargo bloat`, `cargo miri` gates defined in
  AGENTS.md + TETANUS.md but tools not yet installed.

### Phase 1 status: COMPLETE

All 19 crates have real `src/` content (traits + types + error types + unit
tests). No 0-line stubs remain. See the crate table above for per-crate test
counts. Follow-ups (not blockers):

- Real nonlinear quaternion EKF dynamics + Jacobian-based Kalman gain
  (themql-estimation currently uses v0.1 simplified identity-gain placeholder).
- Real SHA-256 (or BLAKE3) in themql-artifact HashValidator (currently a
  placeholder fold hash — must replace before deployment).
- Concrete tch-backed InferenceEngine impl in themql-inference (libtorch build
  dep; trait defined without it for now).
- Wire heavy deps (tch, polars, dioxus, ratatui, embassy) into the
  training/analysis/desktop/embedded crates (currently traits + minimal types
  only, per the task brief).
- Reconcile CacheKey (defined locally in themql-cache) with themql-query's
  CacheKeyer.
- Install cargo-deny, cargo-machete, cargo-bloat, sccache, mold; run the
  safety-critical validation gate.
- Add a CI guard script (TOML parse + crate-name-vs-workspace invariant).
- Write `opencode.json`.