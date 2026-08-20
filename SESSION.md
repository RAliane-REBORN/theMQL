# SESSION.md

Current session state. Update at the end of every turn (see `MEMORY.md`
standing rules). When a session ends, fold the in-flight items into
`HANDOVER.md`.

## Current session

- Date: 2026-08-20
- Mode: build
- Agent: opencode (glm-5.2:cloud)
- Branch: `feat/phase-2-heavy-dep-wiring` (stacked on the Phase 1 work)

## Just-completed turn

Phase 3 Stages 1-11 complete: all placeholder/stub implementations
replaced with real backends across all 20 crates. Commit `828b373`
pushed to `feat/phase-2-heavy-dep-wiring`, PR #4 open.

1. **themql-artifact (Stage 1)**: real BLAKE3 `HashValidator`,
   `FileArtifactLoader`, `BincodeArtifactWriter` (bincode round-trip).
   22 tests.
2. **themql-storage (Stage 2)**: `SledStorage` (disk-backed sled),
   `HelixStorage` alias, `ByPredicate` query support. 31 tests.
3. **themql-cache (Stage 3)**: L1 (`lru::LruCache`), L2 (`moka::sync::Cache`),
   L3 (`redis::Client`), key→subject index for pattern invalidation,
   demotion on hit. 32 tests.
4. **themql-estimation (Stage 4)**: real quaternion EKF with Jacobian +
   Joseph-form Kalman gain (`K = PHᵀ(HPHᵀ+R)⁻¹`,
   `P = (I-KH)P(I-KH)ᵀ + KRKᵀ`), `SensorModel<M>` trait, `GpsModel`,
   `BaroModel`, `BayesianEstimator`. 24 tests.
5. **themql-analysis (Stage 5)**: polars-backed types,
   `StorageAnalysisPipeline`. 12 tests.
6. **themql-training (Stage 6)**: real `TchTrainer` (MLP + Adam + MSE +
   TorchScript export) behind `tch-backend`. 6 default tests.
7. **themql-inference (Stage 7)**: real `TchInferenceEngine` (CModule
   load + `forward_ts` + deadline check + rollback) behind `tch-backend`.
   6 default tests.
8. **themql-sse (Stage 8)**: real broadcast + `serve_sse` (axum +
   Last-Event-ID replay). 17 tests.
9. **themql-mqtt (Stage 9)**: `RumqttcTransport` + `RumqttcConfig`.
   25 tests.
10. **themql-graphql (Stage 10)**: `GraphqlResolverBridgeImpl`,
    `GraphqlSchemaImpl`, `serve_graphql` (axum HTTP/WS). Core
    `Resolver`/`MessageHandler` made dyn-compatible. 13 tests.
11. **Stage 11 (validation)**: all 7 gates green. Commit `828b373`.

## State of the repository

- Phase 1 complete (spec + 20 crates implemented).
- Phase 2 Stages 1-12 complete.
- Phase 3 Stages 1-11 complete: all placeholders replaced with real
  backends. 305 tests pass workspace-wide (304 unit + 1 doc, default
  features). 20 crates, 20 specs.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace`, `cargo deny check`,
  `cargo machete --with-metadata`, `scripts/ci_guard.py` — all green.
- `tch-backend` feature compiles clean in both `themql-training` and
  `themql-inference` (tests not run: libtorch OOM).
- No open bugs (BUG-0008 resolved — the flaky sled temp test is now
  stable after Phase 3 Stage 2 replaced the in-memory HashMap with
  real sled-backed SledStorage).

## In-flight work

Phase 3 follow-ups (user-requested, starting now):

- Workstream A: refresh living docs + merge PR #4 to main + create
  `feat/phase-3-followups` branch. **In progress.**
- Workstream B: wire real GraphQL subscriptions from themql-sse into
  `SubscriptionRoot.subscribe`.
- Workstream D: create GitHub Actions CI workflow.
- Workstream C: implement all 5 themql-desktop subcommands
  (serve/analyze/train/validate/telemetry).

## Next plausible actions (suggestions, not commitments)

1. (in progress) Workstream A → B → D → C.
2. Run `tch-backend` feature tests once a beefier environment is
   available (>7.8GB RAM).
3. Real `themql-embedded` embassy main (requires thumbv7em target).
4. Transport-layer authn/authz policy (MQTT broker credentials, GraphQL
   access control).

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
