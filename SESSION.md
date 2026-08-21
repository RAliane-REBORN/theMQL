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

Phase 9 — core runtime closures:

- **9.1 `DefaultQueryExecutor` orchestrator** in `themql-query`:
  `DefaultQueryExecutor<C: QueryCache>` wraps `Arc<dyn Resolver>`,
  checks `Context.cancellation.is_cancelled()`, checks
  `Context.deadline.is_expired(now)`, optional cache read via
  `QueryCache` trait (new, minimal — `get`/`put`), optional cache
  write-through on miss. `QueryCache` is a new trait that avoids a
  circular dep on `themql-cache`. `DefaultCacheKeyer` used for key
  derivation. +5 tests (invoke on miss, timeout on cancelled,
  write-through, bypass skips read, disabled skips read+write).
- **9.2 Real `EmbassyRuntime::sleep`** in `themql-runtime`:
  replaced `std::future::pending()` with
  `embassy_time::Timer::after(embassy_time::Duration::from_micros(...))`.
  Added `embassy-time` to the `embedded` feature in Cargo.toml.
- **9.3 `StubThedafAdapter`** in `themql-analysis`: concrete impl of
  `ThedafAdapter` returning `AnalysisError::ThedafError("thedaf
  adapter not configured")` for both `fetch_legacy` and
  `list_legacy_datasets`. Gives consumers a type to compose against.
- **9.4 MQTT retained messages** in `themql-mqtt`: added
  `publish_retained` method to `MqttPublisher` trait. The
  `RumqttcTransport` impl passes `retain=true` to the rumqttc
  `publish()` call. Per `specs/mqtt.toml [topics] retained`.
- **9.5 Apalis queue stub** in `themql-desktop`: added `apalis` dep,
  `JobQueue` trait (`enqueue`/`pending_count`), `InMemoryJobQueue`
  stub impl (`Mutex<Vec>`). Real apalis integration (persistent
  storage, workers, retries) is future work.
- 402 tests pass workspace-wide (was 397; +5 from QueryExecutor
  tests). Full validation green.

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
