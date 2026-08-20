# HANDOVER.md

Handover notes for the next agent/session. Fold in-flight items from
`SESSION.md` here when a session ends. Update after every turn (see
`MEMORY.md` standing rules).

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
