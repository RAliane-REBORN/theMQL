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
  `specs/model_artifact.toml` validation gate is the control.
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

## Known limitations (as of v0.1)

- No formal security review process yet.
- No fuzzing harness.
- No dependency audit pipeline.
- No secret-management policy for HelixDB / Valkey credentials.
- No transport-layer authn/authz policy (MQTT broker credentials, GraphQL
  access control). These land in Phase 4 with the transport implementations.

These limitations are tracked; they are not open invitations to land
insecure defaults when the corresponding code is written.