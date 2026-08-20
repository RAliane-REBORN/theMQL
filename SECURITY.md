# SECURITY.md

Security policy and threat model for theMQL. Update after every turn (see
`MEMORY.md` standing rules).

## Reporting

Security issues should be reported via the GitHub issue tracker at
https://github.com/Metis-Avionics/theMQL/issues for now. A dedicated
security-contact channel will be added before v0.2.

Do not file public issues for vulnerabilities that are exploitable in
deployed systems; contact the maintainers privately first.

## Scope

theMQL spans two execution domains with very different security profiles:

- **Desktop** (`themql-desktop` binary) — development, analysis, training,
  telemetry, operations. Runs on developer machines and operations
  infrastructure. Networked. Uses tokio, async-graphql, dioxus, polars,
  tch, helix-db, valkey.
- **Embedded** (`themql-embedded` binary) — vehicle-facing. Runs on
  constrained hardware via embassy. Uses MQTT, sensor inputs, GNC, state
  estimation, ML inference.

A vulnerability in the desktop domain is a research/ops problem. A
vulnerability in the embedded domain can be a safety-of-flight problem.
Treat embedded-domain findings as safety issues, not just security issues.

## Threat model

### Safety-critical (embedded)

- **ML control authority bypass** — any code path where ML silently becomes
  the authoritative flight-control path. Spec-forbidden
  (`SPEC.toml: [ml.authority] ml_control_authority = "forbidden"`). The GNC
  authority hierarchy in `prompts/ARCHITECT.md` is the control.
- **Unvalidated model activation** — activating an embedded model without
  version/schema/compatibility/integrity validation. Spec-forbidden
  (`specs/inference.toml`, `specs/model_artifact.toml`).
- **Euler-angle canonical state** — using Euler angles as the internal
  attitude representation. Spec-forbidden (`specs/gnc.toml`,
  `specs/state_estimation.toml`). Gimbal lock is a safety issue, not a
  style issue.
- **tch in control loop** — coupling the deterministic control path to
  the ML runtime. Spec-forbidden (`specs/gnc.toml:
  tch_rs_in_control_loop = false`).
- **Sensor failure undetected** — any estimator/controller that assumes a
  sensor is permanently valid. Spec-forbidden (`SPEC.toml [safety]`).
- **Online adaptation without budget/rollback** — unconstrained online
  learning. Spec-forbidden (`specs/inference.toml`,
  `prompts/ARCHITECT.md`).
- **Deterministic fallback removed** — any change that makes the
  deterministic GNC/EKF path dependent on ML. Spec-forbidden.

### Security (desktop and transport)

- **Transport business logic leakage** — business logic in MQTT/GraphQL/SSE
  adapters. Spec-forbidden (`SPEC.toml [architecture.forbidden]`). Such
  code is also a maintenance security risk because it bypasses review
  boundaries.
- **Transport-specific domain models** — defining Message/Query/Response
  types inside a transport adapter. Spec-forbidden. Also a deserialization
  attack surface.
- **Implicit cache invalidation** — hidden or implicit cache invalidation
  can cause stale data to be served as authoritative. Spec-forbidden
  (`specs/cache.toml`).
- **Unbounded channels / uncontrolled backpressure** — DoS vector.
  Spec-discouraged (`specs/runtime.toml`).
- **Blocking work on async executor** — can stall the runtime. Spec-forbidden
  (`specs/runtime.toml`).
- **Python ABI / runtime dependency** — not a deliberate attack vector but
  a supply-chain and attack-surface expansion. Spec-forbidden
  (`SPEC.toml [architecture.forbidden]`).
- **Unvalidated model artifact import** — a malicious or corrupt artifact
  could execute arbitrary tensor operations. The
  `specs/model_artifact.toml` validation gate is the control. As of
  Phase 3 Stage 7, `themql-inference::TchInferenceEngine::load()`
  (behind `tch-backend`) runs `themql_artifact::HashValidator::new()
  .validate(&artifact)` and rejects on any `report.errors`, rejects
  empty bytes on every load (not just the first), and only then
  deserialises the bytes into a `tch::CModule`. The placeholder
  behaviour that skipped validation after the first load is gone.
- **HelixDB / Valkey access** — L4 (helix-db) is authoritative; L3 (valkey)
  is distributed ephemeral. Treat credential handling and access boundaries
  as security-relevant from Phase 1 onward.

### Supply chain

- `[workspace.dependencies]` versions are resolved against crates.io
  (2026-08-19). `valkey` is alpha (`0.0.0-alpha5`); `cachelito` is a
  proc-macro for function caching — both may need reassessment. Audit
  transitive deps before production use.
- `deny.toml` enforces license allowlist (MIT, Apache-2.0, BSD, ISC,
  Zlib, CC0), bans Python ABI crates (pyo3, cpython, python3-sys) and
  custom database engines (rusqlite), and restricts sources to crates.io.
  Run `cargo deny check` to verify.
- No `unsafe` is permitted without written justification
  (`SPEC.toml [quality] unsafe_requires_justification = true`). `Miri`
  (`cargo +nightly miri test`) is required for any crate containing
  `unsafe` blocks (see `TETANUS.md` Rule 10).

## Security-relevant specs to consult

- `SPEC.toml` — `[architecture.forbidden]`, `[ml.authority]`, `[safety]`,
  `[quality]`.
- `specs/gnc.toml` — control authority, `tch_rs_in_control_loop = false`.
- `specs/state_estimation.toml` — `ml_failure_must_not_disable_ekf = true`,
  `sensor_failure_detection = true`.
- `specs/inference.toml` — `unvalidated_model_activation = false`,
  `validation_bypass = false`, `rollback_without_retained_previous = false`.
- `specs/model_artifact.toml` — `validation_bypass = false`,
  `schema_bypass = false`, `arbitrary_model_activation = false`.
- `specs/cache.toml` — `implicit_invalidation = false`, `hidden_cache =
  false`.
- `specs/runtime.toml` — `blocking_work_on_async_executor = false`,
  `unbounded_channels_by_default = false`.
- `specs/transport.toml` — `business_logic_in_transport = false`,
  `transport_specific_domain_model = false`.
- `specs/storage.toml` — `business_logic_in_storage = false`,
  `storage_specific_domain_model = false`, `implicit_retry = false`.
- `specs/core.toml` — `no_transport_specific_error_type = true`,
  `response_contains_value_or_error_never_both = true`.

## Security review checklist (per change)

Before a change touches anything in `themql-embedded`, `themql-inference`,
`themql-gnc`, `themql-estimation`, or `themql-telemetry`:

- Does it preserve the deterministic fallback path?
- Does it preserve sensor-failure detection?
- Does it preserve quaternion canonical attitude?
- Does it keep `tch` out of the control loop?
- Does it require ML for basic flight stability? (must not)
- Does it activate a model without the full validation gate? (must not)
- Does it introduce `unsafe`? If yes, is it justified in writing?

Before a change touches anything in `themql-transport`, `themql-graphql`,
`themql-mqtt`, `themql-sse`, `themql-cache`, `themql-storage`:

- Does it add business logic to a transport adapter? (must not)
- Does it define transport-specific domain types that escape the adapter?
  (must not)
- Does it introduce implicit cache invalidation? (must not)
- Does it introduce unbounded channels or blocking work on the async
  executor? (must not)

## Known limitations (as of v0.1, 2026-08-20, Phase 3 Stages 1-11 complete)

All 20 crates now have real `src/` content (traits + types + error types +
unit tests). The four safety-critical crates (themql-gnc, themql-estimation,
themql-inference, themql-artifact) comply with TETANUS.md (Power of Ten
rules): `#![forbid(unsafe_code)]`, `#![deny(warnings)]`, `clippy::pedantic`,
no recursion, fixed loop bounds, no heap alloc after init where required,
functions <= 60 lines, >= 2 assertions per public function as
`if !invariant { return Err }`, no unwrap()/expect() in non-test code.

- **BLAKE3 hash in themql-artifact (real, as of Phase 3 Stage 1)** —
  `HashValidator` uses `blake3::hash` for the 32-byte integrity
  digest (the v0.1 placeholder fold hash was replaced). Model artifacts
  are hash-verified before activation. `TchInferenceEngine::load()`
  (Phase 3 Stage 7) runs the full `HashValidator` validation pipeline
  and rejects on any errors.
- **Real EKF in themql-estimation (as of Phase 3 Stage 4)** — the `Ekf`
  uses real Jacobian-based Kalman gain with Joseph-form covariance
  update (`K = PHᵀ(HPHᵀ+R)⁻¹`, `P = (I-KH)P(I-KH)ᵀ + KRKᵀ`),
  `SensorModel<M>` trait, `GpsModel`, `BaroModel`, and
  `BayesianEstimator`. The v0.1 simplified identity-gain placeholder
  is gone. State estimation correctness is now backed by real
  quaternion dynamics + Jacobian linearisation.
- **21-dim EKF state (fixed, no dynamic allocation)** — `EstimatorState`
  is a fixed 21-dimensional nalgebra `SVector<f64, 21>` with a fixed
  `SMatrix<f64, 21, 21>` covariance. No heap allocation after `new()`. This
  is a safety property (predictable memory, no allocator failure in the
  control loop) and a TETANUS compliance point.
- **TETANUS compliance of gnc/estimation/inference/artifact** — all four
  safety-critical crates carry `#![forbid(unsafe_code)]` and pass the Power
  of Ten rules. No `unsafe` exists anywhere in the workspace.
- **Real tch-backed InferenceEngine (as of Phase 3 Stage 7)** —
  themql-inference has a concrete `TchInferenceEngine` behind the
  `tch-backend` feature that loads a `tch::CModule` (after hash +
  schema validation), runs `forward_ts`, checks the
  `inference_deadline_ms` budget, and supports `rollback()`. ML
  remains augmentative only (never the authoritative flight-control
  path).
- **Real tch-backed Trainer (as of Phase 3 Stage 6)** — themql-training
  has a concrete `TchTrainer` behind `tch-backend` that builds an MLP,
  trains with Adam + MSE, and exports to TorchScript bytes via
  `CModule::create_by_tracing`.
- **EmbassyRuntime sleep stub** — themql-runtime's `EmbassyRuntime` sleep
  is a stub; embassy 0.10 `Spawner` is not `Send`/`Sync` so the
  `EmbeddedRuntime` trait was relaxed from the spec. Confirm before
  relying on the embedded runtime in flight.
- No fuzzing harness.
- No dependency audit pipeline beyond `cargo deny check` (installed and
  passing). `cargo machete` clean. `cargo bloat` passes.
- No secret-management policy for HelixDB / Valkey credentials.
- No transport-layer authn/authz policy (MQTT broker credentials, GraphQL
  access control). These land in Phase 4. The SSE/MQTT/GraphQL adapters
  now have real network I/O surfaces (axum HTTP, rumqttc MQTT client,
  axum HTTP/WS) — transport-layer authn/authz is the next security
  follow-up.
- `themql-cache` has concrete backends wired (L1 lru, L2 moka, L3 redis,
  L4 sled via themql-storage), with a key→subject index for pattern
  invalidation. L3 requires a running `redis-server` (degrades to
  `TierUnavailable` when absent). The trait boundary is where access
  control, credential handling, and input validation must be enforced
  when real valkey/redis L3 and helix-db L4 land.
- `tch` is wired behind a `tch-backend` feature gate in
  themql-training/themql-inference. The feature compiles clean but tests
  are not run in this environment (libtorch + RAM). The ML attack surface
  is real: model bytes are hash-validated + schema-validated before
  deserialisation into a `tch::CModule`, and inference checks the
  `inference_deadline_ms` budget. ML remains augmentative only (never
  the authoritative flight-control path).
- `polars` + `rayon` are wired into themql-analysis. No live analysis
  attack surface yet (RayonAnalysisPipeline runs a trivial parallel
  null-count; StorageAnalysisPipeline queries themql-storage).
- `themql-schema` is a pure type-definition crate with no I/O, no
  network, no unsafe code. No attack surface.

These limitations are tracked; they are not open invitations to land
insecure defaults when the corresponding code is written.
