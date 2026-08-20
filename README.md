# theMQL

The Message Query Language — a native Rust message-query runtime for desktop,
distributed, telemetry, analysis, AI, GNC, and embedded systems.

TheMQL defines a common message and query execution model. External protocols
and applications are projections of that model rather than independent
semantic systems.

## Specifications are authoritative

The repository TOML specifications are the source of truth for architecture,
crate boundaries, dependencies, authority, and safety. The coding agent
prompt is the source of truth for how an implementation must proceed.

- `AGENTS.md` — OpenCode agent entry point (read this first if you are an
  agent; it points at everything else)
- `SPEC.toml` — top-level constitution (v0.1)
- `specs/*.toml` — per-subsystem specifications
- `TETANUS.md` — NASA JPL Power of Ten rules adapted for Rust
  (safety-critical code constraints)
- `prompts/SYSTEM.md` — coding-agent contract entry point
- `prompts/*.md` — role-specific agent prompts

## Repository layout

```
theMQL/
├── SPEC.toml
├── Cargo.toml
├── opencode.json        # opencode config (permissions, commands)
├── rust-toolchain.toml
├── deny.toml            # cargo-deny config (licenses, advisories, bans)
├── TETANUS.md           # NASA JPL Power of Ten rules for Rust
├── .cargo/config.toml   # mold + sccache build config (opt-in)
├── scripts/ci_guard.py  # CI guard (TOML parse + crate-name invariant)
├── specs/               # 20 subsystem specifications
├── prompts/             # coding-agent prompts (six roles)
└── crates/              # 20 crates: 18 libs + 2 binaries
```

## Crates

20 crates: 18 libraries + 2 binaries. ALL 20 now have real `src/` content
(traits + types + error types + unit tests). No 0-line stubs remain.

| Crate | Domain | Role | Tests |
|---|---|---|---|
| `themql-core` | core | semantic owner — canonical Message, Query, Response, Error, Context, Resource types + traits | 31 + 1 doc |
| `themql-schema` | core | canonical shared schema types (FeatureSchema, NormalizationSpec, ModelFormat, TrainedModel, etc.) | 6 |
| `themql-message` | core | Serializer trait + JsonSerializer + MessageError | 4 |
| `themql-query` | core | CacheKey, CacheKeyer, QueryExecutor, Batcher, QueryError | 11 |
| `themql-runtime` | runtime | DesktopRuntime (tokio) + EmbeddedRuntime (embassy), separate traits | 6 default + 8 desktop-feature |
| `themql-cache` | cache | Cache trait, CacheEntry, CacheHit, CacheError; L1 (lru), L2 (moka), L3 (redis), L4 (storage) + key→subject index | 32 |
| `themql-storage` | storage | Storage/Reader/Writer traits, StorageKey/Value/Query/ResultSet; SledStorage (disk-backed), HelixStorage alias | 31 |
| `themql-transport` | transport | Bridge trait, BridgeRoute, TransportKind (sub-specs: mqtt, graphql, sse) | 14 |
| `themql-graphql` | transport | GraphqlSchema/GraphqlResolverBridge traits, Query/Mutation/Subscription #[Object]/#[Subscription] roots, GraphqlResolverBridgeImpl, DispatchBridgeImpl, GraphqlSchemaImpl, serve_graphql (axum) | 13 |
| `themql-mqtt` | transport | MqttTransport/Publisher/Subscriber traits, subject↔topic mapping, codec, RumqttcTransport + RumqttcConfig | 25 |
| `themql-sse` | transport | SseStream/SsePublisher traits, SseEvent wire format, TokioSsePublisher (broadcast), serve_sse (axum + Last-Event-ID) | 17 |
| `themql-telemetry` | telemetry | TelemetryMessage, sensor structs, Covariance [f64;441] | 14 |
| `themql-analysis` | desktop | AnalysisPipeline trait, PolarsDatasetBuilder, RayonAnalysisPipeline, StorageAnalysisPipeline | 12 |
| `themql-training` | desktop | Trainer trait, TchTrainer (real MLP training loop behind tch-backend), Dataset (polars-backed), TrainingConfig | 6 |
| `themql-inference` | embedded | InferenceEngine trait, TchInferenceEngine (real model load + forward pass behind tch-backend), ResourceBudget (TETANUS) | 6 |
| `themql-artifact` | cross-cutting | ArtifactValidator/Loader/Writer, ModelArtifact, HashValidator (real BLAKE3), FileArtifactLoader, BincodeArtifactWriter (TETANUS) | 22 |
| `themql-gnc` | embedded | Controller trait, PID/LQRI/Hybrid, GncState 21-dim (TETANUS) | 7 |
| `themql-estimation` | embedded | Estimator trait, Ekf (Jacobian + Joseph-form Kalman gain), SensorModel, GpsModel, BaroModel, BayesianEstimator, EstimatorState 21-dim (TETANUS) | 24 |
| `themql-embedded` | embedded binary | SensorDriver trait, GpsDriver/BaroDriver/ImuDriver, SensorError 5 variants (TETANUS) | 10 |
| `themql-desktop` | desktop binary | Cli (clap), tokio main, ratatui/crossterm TUI dashboard, 3-pane layout | 13 |

Total: 304 unit tests + 1 doc test pass workspace-wide (default features)
at the Phase 3 close. The `tch-backend` feature compiles clean in
themql-training/themql-inference but tests are not run (libtorch C++ build
needs more RAM than this environment has — 7.8GB, no swap).

## Authority hierarchy

1. `SPEC.toml`
2. `specs/*.toml`
3. tests
4. existing public APIs
5. dependency documentation
6. implementation assumptions

Never silently override a specification.

## Living docs

The repository carries living docs at the root. They are working state, not
historical artifacts. After every turn, the agent updates them (see
`MEMORY.md` for the standing rule):

- `README.md` — project entry point (this file)
- `SECURITY.md` — security policy and threat model
- `SESSION.md` — current session state and in-flight work
- `HANDOVER.md` — handover notes for the next agent/session
- `CHANGELOG.md` — chronological record of changes
- `BUGS.md` — known bugs and unresolved issues
- `MEMORY.md` — persistent facts and standing rules
- `AGENTS_SYNC.md` — coordination log when subagents or multiple agents run

## Status

v0.1 specification drop + Phase 1 complete + Phase 2 Stages 1-12
complete + Phase 3 Stages 1-11 complete. Workspace skeleton, deep
specs, toolchain config, real `src/` content for ALL 20 crates,
cache/storage/transport backends (L1 lru, L2 moka, L3 redis, L4 sled;
SledStorage + HelixStorage alias), runtime impls, desktop/embedded
binary wiring, analysis/training/inference heavy-dep wiring,
cross-crate type reconciliation, safety-critical tooling
(cargo-deny/machete/bloat), opencode.json, CI guard script, a real
GraphQL resolver bridge + axum HTTP/WS integration, a real SSE server
(axum + Last-Event-ID replay), a real MQTT client (rumqttc), a real
tch-backed training loop (themql-training), a real tch-backed model
load + forward pass (themql-inference), a real Jacobian-based EKF with
Joseph-form covariance update (themql-estimation), and real BLAKE3
artifact hashing (themql-artifact) are all in place. Full workspace
validation passes: `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo test --workspace` (305 tests,
default features), `cargo deny check`, `cargo machete --with-metadata`,
`scripts/ci_guard.py`. The `tch-backend` feature compiles clean in
themql-training/themql-inference (tests not run: libtorch OOM). No
`unsafe` code anywhere in the workspace.

The four safety-critical crates (themql-gnc, themql-estimation,
themql-inference, themql-artifact) comply with TETANUS.md (NASA JPL Power
of Ten rules). nalgebra 0.35 is used for all non-ML numerics; `tch` is
forbidden in the GNC control loop and is wired behind a `tch-backend`
feature gate in themql-training/themql-inference (libtorch download +
RAM constraints prevent running `tch-backend` tests in this
environment; the feature compiles clean).

Known v0.1 placeholders (tracked, not blockers): the desktop binary's
serve/analyze/train/validate/telemetry commands print banners only (real
impls are follow-ups); the embedded binary's main is a stub (`embassy
runtime requires thumbv7em target`); `tch-backend` feature tests
(TchTrainer real training loop, TchInferenceEngine real model load +
forward pass) require libtorch and more RAM than this environment
provides — the feature compiles clean in both crates; the GraphQL
`SubscriptionRoot.subscribe` field emits a placeholder stream (real
`themql-message` stream wiring is a follow-up); L1/L3/L4 adapters that
need live infrastructure (redis-server, helix-db HTTP server) degrade
gracefully when the backing service is absent (L3 returns
`TierUnavailable`, L4 falls back to sled disk storage). BUG-0008
(pre-existing flaky `themql-storage::helix_alias_works` sled temp
connection failure) is no longer reproducible after Phase 3 — all 31
storage tests pass cleanly.

The previous v0.0 inline spec that lived in this README has been superseded
by the TOML spec set and is retained only in git history.
## Phase 6 (2026-08-20)

- **Per-request authz**: GraphQL field guards now see the actual
  caller's session role (extracted from better-auth JWT/cookie), not
  a global default. Anonymous callers default to Observer.
- **Dependency audit**: 6 GitHub dependabot alerts classified as
  non-exploitable and ignored in `deny.toml`. `lru` deduped 0.12→0.18.
  See `SECURITY.md` for the full rationale table.
- **Real sensor drivers**: BME280 (I2C barometer with full compensation
  math), LSM6DS3 (I2C IMU), NEO-6M (UART GPS with NMEA 0183 parsing).
  Generic over `embedded-hal` 1.0, mock-tested on host.
- **Embedded MQTT**: `minimq 0.13` (MQTT v5, no_std, async) added.
  Telemetry task formats JSON payloads for the 4 spec subjects.
