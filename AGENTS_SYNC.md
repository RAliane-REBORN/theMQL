# AGENTS_SYNC.md

Coordination log when subagents or multiple agents are running. When no
subagents ran in a turn, leave a one-line "no subagents this turn" entry.
Update after every turn (see `MEMORY.md` standing rules).

## Format

```
### YYYY-MM-DD HH:MM — <lead agent> → <subagent(s)>

- Subagent(s): names / types
- Task: what was delegated
- Outcome: result returned
- Files touched: list
- Conflicts / overlaps: none | description
- Follow-up: none | description
```

## Log

### 2026-08-20 — no subagents this turn (Phase 8 testing infrastructure)

Phase 8 (second phase of the 7-phase sweep on
`feat/phase-7-13-comprehensive`) executed by the lead opencode agent
directly; no subagents were spawned. All 3 sub-steps complete: (8.1)
integration tests in `crates/<crate>/tests/` dirs (4 files, 13 tests);
(8.2) property tests via `proptest` workspace dep (4 files, 19
properties × 64 cases each); (8.3) benchmarks via `criterion` workspace
dep (3 bench files, 9 benchmarks). New workspace deps: `proptest = "1"`,
`criterion = { version = "0.5", features = ["async_tokio"] }`, `tower =
"0.5"` (desktop dev-dep only). 397 tests pass workspace-wide (was 365;
+32).

### 2026-08-20 — no subagents this turn (Phase 7 doc + spec-deviation cleanup)

Phase 7 (first phase of the 7-phase sweep on
`feat/phase-7-13-comprehensive`) executed by the lead opencode agent
directly; no subagents were spawned. All 2 sub-steps complete: (7.1)
refreshed stale HANDOVER/BUGS/MEMORY/SECURITY docs to actual Phase
6-complete state and documented remaining gaps as Phase 8-13 scope;
(7.2) fixed 3 spec deviations in safety-critical crates
(`RollbackHandle.previous_model: TrainedModel`,
`InferenceError::ArtifactInvalid(ArtifactError)`,
`InferenceError::BudgetExceeded { used, limit: ResourceBudget }`,
`TrainingError::ArtifactEmissionFailed(ArtifactError)`). 12 tests pass
in the two affected crates.

### 2026-08-20 — no subagents this turn (Rust 1.98.0 toolchain drift fix)

Toolchain-compatibility fix executed by the lead opencode agent
directly; no subagents were spawned. Rust 1.98.0 stable released during
PR #7 review, introducing `unused_async_trait_impl` clippy lint and
rustfmt drift. Fixed 6 trait impl blocks across 4 crates with
`#[allow(clippy::unused_async_trait_impl)]` + `cargo fmt`. 341 tests
pass workspace-wide.

### 2026-08-20 — no subagents this turn (Phase 5 complete)

Phase 5 (no_std GNC/estimation + embedded EKF/controller + authz, 3
steps) executed by the lead opencode agent directly; no subagents were
spawned. All 3 steps complete: themql-gnc + themql-estimation compile
no_std+alloc, embedded binary runs real EKF + HybridController pipeline,
GraphQL field guards + MQTT topic ACLs implemented. 341 tests pass
workspace-wide.

### 2026-08-20 — no subagents this turn (Phase 4 complete)

Phase 4 (MQTT bridge + auth + embedded embassy main, 4 steps) executed
by the lead opencode agent directly; no subagents were spawned. All 4
steps complete: MQTT-to-SSE bridge wired into serve, better-auth
GraphQL auth + MQTT broker credentials, real embassy embedded main
with 7 tasks cross-compiling for thumbv7em-none-eabihf. 325 tests pass
workspace-wide.

### 2026-08-20 — no subagents this turn (Phase 3 followups complete)

Phase 3 followups (Workstreams A/B/C/D) executed by the lead opencode
agent directly; no subagents were spawned. All 4 workstreams complete:
living docs refreshed + PR #4 merged to main, real GraphQL subscriptions
wired from themql-sse, GitHub Actions CI workflow created, all 5
themql-desktop subcommands implemented with real crate APIs. 306 tests
pass workspace-wide.

### 2026-08-20 — no subagents this turn (earlier)

Phase 3 Stages 1-11 (all placeholder/stub implementations replaced with
real backends across all 20 crates) were executed by the lead opencode
agent directly; no subagents were spawned.

### 2026-08-20 — no subagents this turn

Phase 3 Stages 6 & 7 (real training loop in themql-training + real
model loading/forward pass in themql-inference, both behind the
`tch-backend` feature) were executed by the lead opencode agent
directly; no subagents were spawned.

### 2026-08-20 — no subagents this turn (earlier)

Phase 2 Stage 12 (final validation + commit + PR) was executed by the
lead opencode agent directly; no subagents were spawned.

### 2026-08-20 — no subagents this turn (earlier)

Phase 2 Stage 11 (safety-critical tooling + opencode.json + CI guard)
was executed by the lead opencode agent directly; no subagents were
spawned.

### 2026-08-20 — no subagents this turn (earlier)

Phase 2 Stage 10 (cross-crate type reconciliation: CacheKey +
themql-schema crate) was executed by the lead opencode agent directly;
no subagents were spawned.

### 2026-08-20 — no subagents this turn (earlier)

Phase 2 Stages 2-5 + 9 (transport adapters + analysis/training/inference
heavy-dep wiring) were executed by the lead opencode agent directly; no
subagents were spawned.

### 2026-08-20 — no subagents this turn (earlier)

Phase 2 Stage 1 (themql-cache backends + themql-storage backend) was
executed by the lead opencode agent directly; no subagents were
spawned.

### 2026-08-20 — no subagents this turn (earlier)

Phase 2 Stages 6-8 (themql-runtime, themql-desktop, themql-embedded) were
executed by the lead opencode agent directly; no subagents were spawned.

### 2026-08-20 — opencode (glm-5.2:cloud) → 6 explore/general subagents

- Subagent(s): 6 explore/general subagents this turn (1 spec audit + 5
  parallel implementation subagents for the 18 remaining crates after
  themql-core was implemented manually by the lead).
- Task: Phase 1 — (1) spec audit of all 19 specs, (2) nalgebra amendment,
  (3) deepen all 19 specs from thin/medium to deep with concrete
  types/traits/signatures, (4) implement all 19 crates with real Rust
  source (traits + types + error types + unit tests).
- Outcome: Phase 1 complete. All 19 crates have real `src/` content.
  183 unit tests + 1 doc test = 184 tests, all pass. Full validation
  green: `cargo fmt --all --check`, `cargo check --workspace`, `cargo
  clippy --workspace --all-targets -- -D warnings` (zero warnings),
  `cargo test --workspace`, TOML parse, `cargo metadata`. nalgebra 0.35
  used for all non-ML numerics. `tch` deliberately not added to inference
  (libtorch build dep). EKF uses v0.1 simplified identity-gain
  (placeholder). HashValidator uses a placeholder fold hash (real
  SHA-256 is a follow-up). Heavy deps (tch, polars, dioxus, ratatui,
  embassy) deferred in training/analysis/desktop/embedded. CacheKey
  defined locally in themql-cache; reconcile with themql-query later.
- Files touched: prompts/SYSTEM.md, prompts/ARCHITECT.md, SPEC.toml,
  Cargo.toml, AGENTS.md, all 19 specs/*.toml, all 19
  crates/themql-*/{Cargo.toml,src/*}, + 8 living docs.
- Conflicts / overlaps: none. (Potential future overlap: when
  themql-telemetry grows concrete impls, estimation's local
  ImuReading/GpsReading/BarometerReading should be replaced with
  re-exports. When themql-training is fleshed out, it should re-export
  TrainedModel/FeatureSchema/ModelFormat/ValidationMetrics from
  themql-artifact instead of its current local stubs. CacheKey is
  defined in both themql-cache and themql-query — reconcile later, do
  not duplicate silently.)
- Follow-up: real tch InferenceEngine impl; full nonlinear quaternion
  EKF math; real SHA-256/BLAKE3 in HashValidator; reconcile CacheKey;
  wire heavy deps; install cargo-deny/machete/bloat; CI guard script;
  opencode.json.

### 2026-08-20 — opencode (glm-5.2:cloud), no subagents this turn (earlier)

- Subagent(s): none.
- Task: Phase 1 — implement the four safety-critical crates
  (themql-artifact, themql-gnc, themql-estimation, themql-inference)
  per their specs + TETANUS.md. Read specs (model_artifact.toml,
  gnc.toml, state_estimation.toml, inference.toml), TETANUS.md, and
  themql-core/lib.rs first.
- Outcome: all four crates compile, pass clippy (pedantic +
  deny(warnings), zero warnings), pass fmt --check, and pass tests
  (5 + 7 + 6 + 6 = 24/24). nalgebra 0.35 used for all non-ML numerics.
  `tch` deliberately not added to inference (libtorch build dep);
  InferenceEngine trait defined without it. EKF uses v0.1 simplified
  identity-gain updates (full quaternion dynamics is a future task).
  HashValidator uses a placeholder fold hash (real SHA-256 is a future
  task). Sensor reading types in estimation are minimal local stubs
  matching telemetry schema (canonical types land with
  themql-telemetry).
- Files touched: crates/themql-artifact/{Cargo.toml,src/lib.rs},
  crates/themql-gnc/{Cargo.toml,src/lib.rs},
  crates/themql-estimation/{Cargo.toml,src/lib.rs},
  crates/themql-inference/{Cargo.toml,src/lib.rs}, + 8 living docs.
- Conflicts / overlaps: none. (Potential future overlap: when
  themql-telemetry is implemented, estimation's local
  ImuReading/GpsReading/BarometerReading should be replaced with
  re-exports. When themql-training is fleshed out, it should re-export
  TrainedModel/FeatureSchema/ModelFormat/ValidationMetrics from
  themql-artifact instead of its current local stubs.)
- Follow-up: real tch InferenceEngine impl; full EKF math; real
  SHA-256 in HashValidator; themql-telemetry for canonical sensor
  types.

### 2026-08-20 — opencode (glm-5.2:cloud), no subagents this turn (earlier)

- Subagent(s): none.
- Task: Phase 1 — implement themql-cache, themql-transport,
  themql-storage (traits + types + error types only). Read specs
  (cache.toml, transport.toml, storage.toml) and themql-core/lib.rs
  first; implemented per spec API surfaces; no concrete backends.
- Outcome: all three crates compile, pass clippy (pedantic +
  deny(warnings), zero warnings), and pass tests (16 + 14 + 23 = 53/53).
  `cargo fmt --check` clean. `helix-db` v3.0.0 compiles but is
  intentionally unused in themql-storage. `CacheKey` defined locally in
  themql-cache (no hard dep on the not-yet-implemented themql-query).
- Files touched: crates/themql-cache/{Cargo.toml,src/lib.rs},
  crates/themql-transport/{Cargo.toml,src/lib.rs},
  crates/themql-storage/{Cargo.toml,src/lib.rs}, + 8 living docs.
- Conflicts / overlaps: none. (Potential future overlap: when
  themql-query implements CacheKeyer, reconcile its CacheKey with
  themql-cache's local CacheKey — do not duplicate.)
- Follow-up: wire concrete backends (cachelito/moka/valkey/helix-db)
  behind the new traits in a later phase.

### 2026-08-19 — opencode (glm-5.2:cloud), no subagents this turn

- Subagent(s): none.
- Task: toolchain hardening. Updated rust-toolchain.toml (components +
  targets), created .cargo/config.toml (mold + sccache opt-in),
  created deny.toml (cargo-deny), created TETANUS.md (NASA JPL Power of
  Ten adapted for Rust). Updated AGENTS.md with safety-critical
  validation commands and TETANUS summary. Added profiles to Cargo.toml.
- Outcome: 43/43 TOML parse, cargo check --workspace passes for all 19
  crates. Tools (mold, sccache, cargo-deny, cargo-machete, cargo-bloat)
  not installed in this environment; config files are ready for when they
  are.
- Files touched: rust-toolchain.toml, .cargo/config.toml (new), deny.toml
  (new), TETANUS.md (new), Cargo.toml, AGENTS.md, + 8 living docs.
- Conflicts / overlaps: none.
- Follow-up: install tools and run the full safety-critical validation
  gate when available.

### 2026-08-19 — opencode (glm-5.2:cloud), no subagents this turn (earlier)

- Subagent(s): none.
- Task: v0.1 specification + workspace skeleton drop, then living-docs
  adoption.
- Outcome: all work done directly by the lead agent. No subagent
  coordination required.
- Files touched: see `CHANGELOG.md` 2026-08-19 entries for the full list.
- Conflicts / overlaps: none.
- Follow-up: none.
### 2026-08-20 — opencode (glm-5.2:cloud), no subagents this turn (earlier still)

- Subagent(s): none.
- Task: implement trait + type stubs for `themql-training`,
  `themql-analysis`, `themql-desktop` (binary), `themql-embedded`
  (binary) from their specs, without heavy deps.
- Outcome: all 4 crates implemented, pass fmt + clippy (-D warnings) +
  test (19 tests). `themql-artifact`/`themql-telemetry`/`themql-gnc`/
  `themql-estimation` were still empty stubs at that point; dependent
  types were defined locally per the task brief. Pre-existing
  `themql-mqtt` breakage was observed but not touched.
- Files touched: `crates/themql-training/{Cargo.toml,src/lib.rs}`,
  `crates/themql-analysis/{Cargo.toml,src/lib.rs}`,
  `crates/themql-desktop/{Cargo.toml,src/main.rs}`,
  `crates/themql-embedded/{Cargo.toml,src/main.rs}`,
  `CHANGELOG.md`, `SESSION.md`, `AGENTS_SYNC.md`.
- Conflicts / overlaps: none.
- Follow-up: none (superseded by the Phase 1 complete turn above).

## 2026-08-20

- No subagents this turn. themql-sse Stage 8 implemented directly
  (real broadcast channel + axum serve_sse + Last-Event-ID replay).

## 2026-08-20 (Phase 6)

- No subagents this turn. All 4 steps (per-request authz, dependabot
  remediation + lru dedup, real sensor drivers, embedded MQTT)
  implemented directly. Single PR (#8), 4 commits.
