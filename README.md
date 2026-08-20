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
| `themql-cache` | cache | Cache trait, CacheEntry, CacheHit, CacheError (L1-L4 tiered) | 30 |
| `themql-storage` | storage | Storage/Reader/Writer traits, StorageKey/Value/Query/ResultSet | 29 |
| `themql-transport` | transport | Bridge trait, BridgeRoute, TransportKind (sub-specs: mqtt, graphql, sse) | 14 |
| `themql-graphql` | transport | GraphqlSchema/GraphqlResolverBridge traits, Query/Mutation #[Object] roots | 6 |
| `themql-mqtt` | transport | MqttTransport/Publisher/Subscriber traits, subject↔topic mapping, codec | 14 |
| `themql-sse` | transport | SseStream/SsePublisher traits, SseEvent wire format, broadcast-backed | 13 |
| `themql-telemetry` | telemetry | TelemetryMessage, sensor structs, Covariance [f64;441] | 14 |
| `themql-analysis` | desktop | AnalysisPipeline trait, PolarsDatasetBuilder, RayonAnalysisPipeline | 9 |
| `themql-training` | desktop | Trainer trait, TchTrainer (tch-backend feature), Dataset, TrainingConfig | 5 |
| `themql-inference` | embedded | InferenceEngine trait, TchInferenceEngine (tch-backend feature), ResourceBudget (TETANUS) | 6 |
| `themql-artifact` | cross-cutting | ArtifactValidator/Loader/Writer, ModelArtifact (TETANUS) | 5 |
| `themql-gnc` | embedded | Controller trait, PID/LQRI/Hybrid, GncState 21-dim (TETANUS) | 7 |
| `themql-estimation` | embedded | Estimator trait, Ekf, EstimatorState 21-dim (TETANUS) | 6 |
| `themql-embedded` | embedded binary | SensorDriver trait, GpsDriver/BaroDriver/ImuDriver, SensorError 5 variants (TETANUS) | 10 |
| `themql-desktop` | desktop binary | Cli (clap), tokio main, ratatui/crossterm TUI dashboard, 3-pane layout | 13 |

Total: 240 tests pass workspace-wide (default features) at Phase 2
Stage 10 close. The `tch-backend` feature in themql-training/
themql-inference compiles but tests are not run (libtorch download +
RAM constraints in this environment).

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
complete. Workspace skeleton, deep specs, toolchain config, real `src/`
content for ALL 20 crates, cache/storage/transport backends, runtime
impls, desktop/embedded binary wiring, analysis/training/inference
heavy-dep wiring, cross-crate type reconciliation, safety-critical
tooling (cargo-deny/machete/bloat), opencode.json, and CI guard script
are all in place. Full workspace validation passes: `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test
--workspace` (240 tests, default features), `cargo deny check`, `cargo
machete --with-metadata`, `scripts/ci_guard.py`.

The four safety-critical crates (themql-gnc, themql-estimation,
themql-inference, themql-artifact) comply with TETANUS.md (NASA JPL Power
of Ten rules). nalgebra 0.35 is used for all non-ML numerics; `tch` is
forbidden in the GNC control loop and is wired behind a `tch-backend`
feature gate in themql-training/themql-inference (libtorch download +
RAM constraints prevent running `tch-backend` tests in this
environment; the feature compiles clean).

Known v0.1 placeholders (tracked, not blockers): the EKF uses simplified
identity-gain updates (full quaternion dynamics is a follow-up); the
artifact HashValidator uses a placeholder fold hash (real SHA-256 is a
follow-up, required before deployment); the desktop binary's
serve/analyze/train/validate/telemetry commands print banners only (real
impls are follow-ups); the embedded binary's main is a stub (`embassy
runtime requires thumbv7em target`); `tch-backend` feature tests
(TchTrainer, TchInferenceEngine) require libtorch and more RAM than this
environment provides; cachelito L1 / valkey L3 / helix-db L4 adapters
are stubs (cachelito API mismatch, valkey alpha driver, helix-db needs
live server); CacheKey is defined locally in themql-cache (reconcile
with themql-query's CacheKeyer — Stage 10).

The previous v0.0 inline spec that lived in this README has been superseded
by the TOML spec set and is retained only in git history.