# SESSION.md

Current session state. Update at the end of every turn (see `MEMORY.md`
standing rules). When a session ends, fold the in-flight items into
`HANDOVER.md`.

## Current session

- Date: 2026-08-20
- Mode: build
- Agent: opencode (glm-5.2:cloud)
- Branch: `feat/phase-7-13-comprehensive` (off main, post PR #8 merge)

## Just-completed turn

Phase 8 — testing infrastructure (integration/property/bench):

- **8.1 Integration tests**: 4 test files in `crates/<crate>/tests/`
  dirs: `themql-cache/tests/tiered_flow.rs` (4 tests: L4 sled promotes
  to L1, invalidate removes from all, disabled policy misses, bypass
  policy skips read but writes through), `themql-storage/tests/
  sled_round_trip.rs` (5 tests: put/get, delete, missing, query-by-key,
  overwrite), `themql-graphql/tests/resolver_subscription_e2e.rs` (2
  tests: GraphQL query through resolver bridge, GraphQL subscription
  streams events from SSE publisher), `themql-desktop/tests/serve_boot.rs`
  (2 tests: POST /graphql returns 200, GET /events returns 200; uses
  `tower::ServiceExt::oneshot`).
- **8.2 Property tests**: 4 test files via `proptest` workspace dep:
  `themql-core/tests/subject_property.rs` (7 properties: round-trip,
  clone, is_concrete, Display, pattern-matches-self, wildcard-multi,
  wildcard-one), `themql-query/tests/cache_key_property.rs` (5
  properties: deterministic, distinct-subjects, hash_of-deterministic,
  hash_of-distinct-input, Display-is-hex-64), `themql-estimation/tests/
  ekf_property.rs` (5 properties × 64 cases: covariance symmetry after
  predict/update_gps/update_baro, quaternion norm after predict,
  nonpositive-dt rejection), `themql-gnc/tests/hybrid_property.rs` (2
  properties × 64 cases: finite output for bounded inputs, nonpositive-dt
  rejection).
- **8.3 Benchmarks**: 3 bench dirs via `criterion` workspace dep:
  `themql-estimation/benches/ekf_step.rs` (3 benches: predict,
  predict+gps_update, predict+baro_update), `themql-cache/benches/
  tiered.rs` (5 benches: l1_get_hit, l1_put, l2_get_hit,
  tiered_l1_hit_async, tiered_l4_sled_hit_async), `themql-graphql/benches/
  resolve.rs` (1 bench: graphql_resource_query).
- New workspace deps: `proptest = "1"`, `criterion = { version = "0.5",
  features = ["async_tokio"] }`, `tower = "0.5"` (desktop dev-dep only).
- 397 tests pass workspace-wide (was 365; +32 from integration +
  property tests). 0 failures. Full validation green: fmt, check,
  clippy, test, deny, machete, TOML sanity, ci-guard, metadata,
  embedded cross-compile (thumbv7em-none-eabihf).

## State of the repository

- Phase 1 complete (spec + 20 crates implemented).
- Phase 2 Stages 1-12 complete.
- Phase 3 Stages 1-11 complete.
- Phase 3 followups complete (PR #5 merged).
- Phase 4 complete (PR #6 merged): MQTT-to-SSE bridge, better-auth,
  embassy embedded main.
- Phase 5 complete: no_std gnc/estimation, real EKF+controller in
  embedded binary, GraphQL authz guards + MQTT topic ACLs.
- Phase 6 complete: per-request authz + dep audit + real sensors +
  embedded MQTT (PR #8 merged).
- Phase 7 complete: doc + spec-deviation cleanup.
- 365 tests pass workspace-wide (default features). 20 crates, 21 specs.
- Full validation green.
- `tch-backend` feature compiles clean (tests not run: libtorch OOM).
- Embedded binary cross-compiles for thumbv7em-none-eabihf with real
  EKF + HybridController + sensor drivers + minimq MQTT payload
  formatting.
- No open bugs.

## In-flight work

Phase 7 of a 7-phase sweep (Phases 7-13) on
`feat/phase-7-13-comprehensive`. Phases 8-13 pending: testing
infrastructure, core runtime closures, safety-critical mechanisms,
embedded networking, training pipeline completeness, desktop dioxus UI +
pnpm toolchain.

## Next plausible actions (suggestions, not commitments)

1. Phase 8: integration tests in `crates/<crate>/tests/`, property tests
   via `proptest`, benchmarks via `criterion`.
2. Phase 9: `QueryExecutor` orchestrator, real `EmbassyRuntime::sleep`,
   `ThedafAdapter` stub, MQTT retained messages, apalis queue stub.
3. Phase 10: controller/estimator/ML failure detection + ML-degrade-to-
   EKF-only fallback.
4. Phase 11: `embassy-net` TCP transport + embedded MQTT publish +
   command subscribe.
5. Phase 12: TrainerKind dispatch + pruning + sparsification + online
   adaptation trait method (compile-only, tch-backend tests deferred).
6. Phase 13: dioxus 0.7 fullstack UI + pnpm/Tailwind/Playwright
   toolchain + SPEC.toml amendment for pnpm carve-out.

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

## Phase 6 (2026-08-20): Per-request authz + dep audit + sensors + MQTT

4 commits on branch `feat/phase-6-authz-sensors-mqtt`, single PR #8.

1. **Per-request GraphQL role extraction**: Custom axum handler in
   `themql-desktop` extracts `better-auth` session → `AuthRole` →
   injects per-request via `BatchRequest::data()`. When auth off,
   guards reject all requests (no global role). `AuthRole: FromStr`.
2. **Dependabot remediation**: All 6 GitHub alerts are non-exploitable
   (rustls-webpki CRL/X.509 paths not activated, jsonwebtoken type
   confusion on JWT path not used, lru IterMut Stacked-Borrows only).
   `lru` 0.12→0.18 dedup. 6 advisories ignored in `deny.toml`.
3. **Real sensor drivers**: BME280 (I2C, full compensation math),
   LSM6DS3 (I2C IMU), NEO-6M (UART NMEA parser). Generic over
   `embedded-hal` 1.0. `SensorDriver` trait now `async fn read`.
4. **Embedded MQTT**: `minimq 0.13` added. Spec amended. Telemetry
   task formats JSON payloads via hand-formatted `write_f64`/`write_u32`.

365 tests pass. Full validation green: fmt, check, test, clippy, deny,
TOML sanity, metadata, cross-compile (thumbv7em-none-eabihf).
