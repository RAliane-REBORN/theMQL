# SESSION.md

Current session state. Update at the end of every turn (see `MEMORY.md`
standing rules). When a session ends, fold the in-flight items into
`HANDOVER.md`.

## Current session

- Date: 2026-08-20
- Mode: build
- Agent: opencode (glm-5.2:cloud)
- Branch: `feat/phase-5-no_std-authz` (off main, post PR #6 merge)

## Just-completed turn

Phase 5 — no_std GNC/estimation, embedded EKF+controller wiring, authz (3 steps):

1. **Step 1 — no_std migration of themql-gnc + themql-estimation**: Both
   crates now compile with `--no-default-features` (no_std + alloc).
   `default = ["std"]` feature gates `themql-core` + `thiserror/std`.
   `nalgebra` with `default-features = false` + `libm` for float
   transcendentals. `num-traits` with `libm` for `Real` trait
   (estimation, no_std mode only). `extern crate alloc;` +
   `use alloc::string::String / vec::Vec`. `core::fmt` / `core::ptr`
   instead of `std::fmt` / `std::ptr`. `#[cfg(feature = "std")]` gate on
   `From<Error> for themql_core::Error`. Tests pass in both modes.

2. **Step 2 — Wire real EKF + HybridController into embedded binary**:
   The embedded binary now runs the full GNC pipeline on thumbv7em.
   Global allocator (`embedded-alloc::TlsfHeap`, 16KB heap) for alloc
   usage. `estimator_task`: real EKF predict (IMU) + update (GPS/baro).
   `controller_task`: real `HybridController` consuming
   `EstimatorState`. `estimator_to_gnc_state` free function maps 21-dim
   state to `GncState`. `EST_CHAN`: estimator→controller channel.
   Sensor decode functions (stub drivers → zero readings). Single
   `unsafe` block in `main()` for allocator init (justified in TETANUS
   docs). `forbid(unsafe_code)` → `deny(unsafe_code)` with local allow.
   +3 host tests (19 total).

3. **Step 3 — GraphQL authz + MQTT ACLs**: `AuthRole` enum (Admin,
   Operator, Observer) with hierarchy. `RoleGuard` implementing
   `async_graphql::Guard`. QueryRoot fields guarded with Observer,
   MutationRoot with Operator, SubscriptionRoot with Observer.
   `GraphqlSchemaImpl::with_role()` injects role as global data. MQTT
   `AclAction` / `AclRule` / `MqttAcl` with topic pattern matching
   (`*` wildcard). `admin_acl`/`operator_acl`/`observer_acl` presets.
   `RumqttcConfig.with_acl()` + `RumqttcTransport` checks ACL before
   publish/subscribe. Desktop: `serve()` uses `with_role(Admin)`, MQTT
   username → ACL mapping. +17 authz tests across crates.

**Totals**: 341 tests pass workspace-wide (was 325). Full validation
green: fmt, check, clippy, test, deny, machete, embedded-check,
toml-sanity, ci-guard.

## State of the repository

- Phase 1 complete (spec + 20 crates implemented).
- Phase 2 Stages 1-12 complete.
- Phase 3 Stages 1-11 complete.
- Phase 3 followups complete (PR #5 merged).
- Phase 4 complete (PR #6 merged): MQTT-to-SSE bridge, better-auth,
  embassy embedded main.
- Phase 5 complete: no_std gnc/estimation, real EKF+controller in
  embedded binary, GraphQL authz guards + MQTT topic ACLs.
- 341 tests pass workspace-wide (default features). 20 crates, 21 specs.
- Full validation green.
- `tch-backend` feature compiles clean (tests not run: libtorch OOM).
- Embedded binary cross-compiles for thumbv7em-none-eabihf with real
  EKF + HybridController.
- No open bugs.

## In-flight work

None. Phase 5 is complete. Changes are committed on
`feat/phase-5-no_std-authz` branch, ready to push and open PR.

## Next plausible actions (suggestions, not commitments)

1. Merge Phase 5 PR to main after CI green.
2. Run `tch-backend` feature tests once a beefier environment is
   available (>7.8GB RAM).
3. Per-request role extraction from better-auth sessions (currently
   uses default Admin role; middleware to extract role from JWT
   session and inject per-request).
4. Fuzzing harness + secret-management policy.
5. Real sensor drivers (I2C/SPI/UART) for embedded binary.
6. MQTT publish path in embedded binary (currently telemetry task
   counts cycles only).

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
