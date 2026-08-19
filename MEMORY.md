# MEMORY.md

Living memory for theMQL. Persistent facts and rules that must survive across
sessions and across agents. Update after every turn.

## Standing rules

### Living-docs rule (added 2026-08-19)

After **every turn**, update the living docs at the repository root:

- `README.md` — project entry point
- `SECURITY.md` — security policy and threat model
- `SESSION.md` — current session state and in-flight work
- `HANDOVER.md` — handover notes for the next agent/session
- `CHANGELOG.md` — chronological record of changes
- `BUGS.md` — known bugs and unresolved issues
- `MEMORY.md` — this file
- `AGENTS_SYNC.md` — coordination log when subagents or multiple agents are
  running (leave a "no subagents this turn" entry otherwise)

These are not historical artifacts. They are working state. If a turn ends
and any of these are stale, the turn is not finished.

### Authority hierarchy (from SPEC.toml)

1. `SPEC.toml`
2. `specs/*.toml`
3. tests
4. existing public APIs
5. dependency documentation
6. implementation assumptions

Never silently override a specification.

### Composition over reimplementation

theMQL is a composition system. If a mature dependency already provides the
required primitive, use that primitive. Do not recreate it. See
`prompts/SYSTEM.md` for the full dependency policy.

### Training/inference split (v0.1)

`themql-ai` does not exist and must not be introduced. Model creation lives in
`themql-training` (desktop); model execution lives in `themql-inference`
(embedded). The artifact boundary is governed by `specs/model_artifact.toml`
and enforced by `themql-artifact` (validation crate).

### GNC authority

Basic vehicle stability must remain possible without machine learning. The
authority hierarchy is: deterministic GNC → EKF/state estimation → Bayesian
reasoning → ML-assisted state correction → ML prediction/anomaly detection. ML
must not silently become the authoritative flight-control path.

### Canonical attitude representation

Quaternion. Euler angles may be generated for UI/logging/debugging only.

### tch-rs in control loop

Forbidden. `specs/gnc.toml` enforces `tch_rs_in_control_loop = false`. This is
deliberate, not an oversight.

## Persistent facts

### Project

- Name: theMQL (The Message Query Language)
- Version: 0.1.0
- Status: greenfield
- Language: Rust
- License: MIT
- Repository: https://github.com/Metis-Avionics/theMQL
- Toolchain: stable channel, no version pin
- Workspace resolver: 2

### Crates (19)

17 library crates + 2 binary crates:

| Crate | Domain | Role |
|---|---|---|
| themql-core | core | semantic owner — canonical types (Message, Query, Response, Error, Context, Resource) |
| themql-message | core | message routing + serialization (subordinate to core.toml) |
| themql-query | core | query resolution + caching (subordinate to core.toml) |
| themql-runtime | runtime | tokio/embassy execution, apalis, rayon |
| themql-cache | cache | L1-L4 tiered cache orchestration |
| themql-storage | storage | helix-db authoritative storage adapter |
| themql-transport | transport | cross-cutting bridges + constraints |
| themql-graphql | transport | async-graphql projection |
| themql-mqtt | transport | embassy MQTT transport |
| themql-sse | transport | server-sent events projection |
| themql-telemetry | telemetry | first-class telemetry message schema |
| themql-analysis | desktop | polars analytical processing |
| themql-training | desktop | tch training + artifact generation |
| themql-inference | embedded | tch inference + online adaptation |
| themql-artifact | cross-cutting | model artifact validation + transfer |
| themql-gnc | embedded | deterministic PID/LQRI/hybrid control |
| themql-estimation | embedded | EKF + Bayesian + quaternion state |
| themql-embedded | binary | embassy embedded binary |
| themql-desktop | binary | dioxus + graphql + training binary |

### Dependency stack (resolved against crates.io 2026-08-19)

- serde 1, serde_json 1
- tokio 1, rayon 1, apalis 0.6
- async-graphql 8.0.0-rc.5
- embassy-executor 0.10, embassy-sync 0.8
- cachelito 0.16 (L1), moka 0.12 (L2), valkey 0.0.0-alpha5 (L3), helix-db 3 (L4/storage)
- dioxus 0.5 (UI), polars 0.55 (analysis)
- tch 0.24 (ML tensors — published as `tch`, not `tch-rs`), ndarray 0.17
- clap 4 (CLI), ratatui 0.30 (TUI)

Note: `valkey` is alpha; `cachelito` is a proc-macro for function caching
(may need reassessment for L1 use case in Phase 1).

### Canonical sensors

- NEO-6M GPS
- BME280 barometer
- GY-LSM6DS3 IMU

### theDAF integration

`Metis-Avionics/theDAF` is an integration-only component for legacy data
access, analysis support, and migration reference. theMQL core must not depend
on theDAF. the embedded binary must not depend on theDAF.

## Decision log (chronological)

- 2026-08-19: v0.1 spec drop. Split `themql-ai` into `themql-training` (desktop)
  + `themql-inference` (embedded). README rewritten as pointer (v0.0 inline
  spec retained only in git history). Prompts split into six role files.
- 2026-08-19: Adopted living-docs rule. After every turn, update README,
  SECURITY, SESSION, HANDOVER, CHANGELOG, BUGS, MEMORY, AGENTS_SYNC.
- 2026-08-19: Created `AGENTS.md` — OpenCode convention file. Compact
  restatement of authority hierarchy, validation commands, architectural
  constraints, dependency policy, workspace layout, forbidden patterns,
  final report format. Points at `prompts/SYSTEM.md` as the full contract.
- 2026-08-19: Completed subsystem spec set. Wrote 5 new deep specs
  (core.toml, storage.toml, graphql.toml, mqtt.toml, sse.toml). Rewrote
  message.toml + query.toml as subordinate to core.toml (canonical type
  definitions moved to core.toml). Trimmed transport.toml to bridges +
  cross-cutting constraints. Expanded model_artifact.toml with new
  `themql-artifact` crate (19 total). Resolved all workspace dep versions
  against crates.io. Wired up 6 crate Cargo.tomls with real deps.
  `cargo check --workspace` passes for all 19 crates.
- 2026-08-19: Toolchain hardening. Updated `rust-toolchain.toml` with
  components (rustfmt, clippy, rust-src, miri) and embedded targets
  (thumbv7em, riscv32imc). Created `.cargo/config.toml` (mold + sccache
  configs, opt-in via comments; cargo aliases). Created `deny.toml`
  (cargo-deny: licenses, advisories, bans, sources — bans pyo3/cpython/
  rusqlite). Created `TETANUS.md` — NASA JPL Power of Ten rules adapted
  for Rust, with enforcement mechanisms and safety-critical crate
  identification. Updated `AGENTS.md` with safety-critical validation
  commands (cargo deny, machete, bloat, miri) and TETANUS summary.
  Added `[profile.dev]`, `[profile.release]`, `[profile.bench]` to root
  Cargo.toml.

### Validation baseline

At end of 2026-08-19 (subsystem spec set + dep wiring + toolchain hardening):

- 43/43 TOML files parse via `python3 tomllib`.
- `cargo metadata --no-deps --format-version 1` resolves.
- `cargo check --workspace` passes for all 19 crates with real deps.
- `cargo fmt --check`, `cargo clippy`, `cargo deny`, `cargo machete`,
  `cargo bloat`, `cargo miri` gates defined in AGENTS.md + TETANUS.md.