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
├── rust-toolchain.toml
├── deny.toml           # cargo-deny config (licenses, advisories, bans)
├── TETANUS.md          # NASA JPL Power of Ten rules for Rust
├── .cargo/config.toml  # mold + sccache build config (opt-in)
├── specs/              # 19 subsystem specifications
├── prompts/            # coding-agent prompts (six roles)
└── crates/             # 19 crates: 17 libs + 2 binaries
```

## Crates

19 crates: 17 libraries + 2 binaries. ALL 19 now have real `src/` content
(traits + types + error types + unit tests). No 0-line stubs remain.

| Crate | Domain | Role | Tests |
|---|---|---|---|
| `themql-core` | core | semantic owner — canonical Message, Query, Response, Error, Context, Resource types + traits | 31 + 1 doc |
| `themql-message` | core | Serializer trait + JsonSerializer + MessageError | 4 |
| `themql-query` | core | CacheKey, CacheKeyer, QueryExecutor, Batcher, QueryError | 11 |
| `themql-runtime` | runtime | DesktopRuntime (tokio) + EmbeddedRuntime (embassy), separate traits | 6 |
| `themql-cache` | cache | Cache trait, CacheEntry, CacheHit, CacheError (L1-L4 tiered) | 16 |
| `themql-storage` | storage | Storage/Reader/Writer traits, StorageKey/Value/Query/ResultSet | 23 |
| `themql-transport` | transport | Bridge trait, BridgeRoute, TransportKind (sub-specs: mqtt, graphql, sse) | 14 |
| `themql-graphql` | transport | GraphqlSchema/GraphqlResolverBridge traits, Query/Mutation/Subscription roots | 4 |
| `themql-mqtt` | transport | MqttTransport/Publisher/Subscriber traits, MqttQos, SubscriptionId | 9 |
| `themql-sse` | transport | SseStream/SsePublisher traits, SseEvent, SseError | 8 |
| `themql-telemetry` | telemetry | TelemetryMessage, sensor structs, Covariance [f64;441] | 14 |
| `themql-analysis` | desktop | AnalysisPipeline trait, DatasetBuilder, ThedafAdapter | 5 |
| `themql-training` | desktop | Trainer trait, Dataset, TrainingConfig, TrainedModel | 5 |
| `themql-inference` | embedded | InferenceEngine trait, ResourceBudget, RollbackHandle (TETANUS) | 6 |
| `themql-artifact` | cross-cutting | ArtifactValidator/Loader/Writer, ModelArtifact (TETANUS) | 5 |
| `themql-gnc` | embedded | Controller trait, PID/LQRI/Hybrid, GncState 21-dim (TETANUS) | 7 |
| `themql-estimation` | embedded | Estimator trait, Ekf, EstimatorState 21-dim (TETANUS) | 6 |
| `themql-embedded` | embedded binary | SensorDriver trait, embassy task topology (TETANUS) | 4 |
| `themql-desktop` | desktop binary | Cli (clap), Command enum, main entry | 5 |

Total: 183 unit tests + 1 doc test = 184 tests, all green.

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

v0.1 specification drop + Phase 1 complete. Workspace skeleton, deep specs,
toolchain config, and real `src/` content for ALL 19 crates are in place.
183 unit tests + 1 doc test = 184 tests, all green. Full validation passes:
`cargo fmt --all --check`, `cargo check --workspace`, `cargo clippy
--workspace --all-targets -- -D warnings` (zero warnings), `cargo test
--workspace` (184 pass), `python3 tomllib` (all TOML parse), `cargo metadata
--no-deps` (resolves).

The four safety-critical crates (themql-gnc, themql-estimation,
themql-inference, themql-artifact) comply with TETANUS.md (NASA JPL Power of
Ten rules). nalgebra 0.35 is used for all non-ML numerics; `tch` is forbidden
in the GNC control loop and is deliberately not yet wired into inference
(libtorch build dep deferred).

Known v0.1 placeholders (tracked, not blockers): the EKF uses simplified
identity-gain updates (full quaternion dynamics is a follow-up); the
artifact HashValidator uses a placeholder fold hash (real SHA-256 is a
follow-up, required before deployment); heavy deps (tch, polars, dioxus,
ratatui, embassy) are deferred in training/analysis/desktop/embedded (traits
+ minimal types only); concrete cache/storage/transport backends are not yet
wired behind the trait surfaces.

The previous v0.0 inline spec that lived in this README has been superseded
by the TOML spec set and is retained only in git history.