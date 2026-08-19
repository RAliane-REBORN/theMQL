# CHANGELOG.md

Chronological record of changes to theMQL. Newest entries at the top.
Update after every turn (see `MEMORY.md` standing rules).

## [Unreleased]

### 2026-08-19 — Toolchain hardening + TETANUS.md

Hardened the build toolchain: rust-toolchain components + embedded targets,
cargo-deny config, mold/sccache build config, and the NASA JPL Power of Ten
rules codified as `TETANUS.md`.

#### Added — build toolchain config

- `rust-toolchain.toml` — expanded with `components` (rustfmt, clippy,
  rust-src, miri) and `targets` (x86_64-unknown-linux-gnu, thumbv7em-none-eabi,
  thumbv7em-none-eabihf, riscv32imc-unknown-none-elf).
- `.cargo/config.toml` — mold linker + sccache wrapper configs (opt-in via
  comments; do not break builds when tools absent). Cargo aliases: `chk`,
  `tst`, `fmtc`, `clip`, `deny`, `machete`.
- `deny.toml` — cargo-deny configuration. License allowlist (MIT, Apache-2.0,
  BSD, ISC, Zlib, CC0, etc.). Bans: pyo3, cpython, python3-sys (Python ABI),
  rusqlite (custom database). Advisory checking via RustSec db. Source
  restriction to crates.io only.
- `TETANUS.md` — NASA JPL Power of Ten rules adapted for Rust. Full
  rationale, enforcement mechanisms, per-rule Rust-specific guidance,
  summary table, and safety-critical crate identification (themql-gnc,
  themql-estimation, themql-inference, themql-artifact).

#### Changed — workspace config

- `Cargo.toml` — added `[profile.dev]` (split-debuginfo), `[profile.release]`
  (lto=thin, codegen-units=1, strip=symbols), `[profile.bench]` (lto=thin,
  codegen-units=1).
- `AGENTS.md` — added safety-critical validation section (cargo deny, machete,
  bloat, miri), TETANUS.md summary (10 rules), updated TOML file count to 40+,
  updated workspace crate count to 19, updated dependency names (tch, helix-db).

#### Validation

- 43/43 TOML files parse.
- `cargo metadata --no-deps` resolves.
- `cargo check --workspace` passes for all 19 crates.
- `cargo deny`, `cargo machete`, `cargo bloat` gates defined but not yet
  executable (tools not installed in this environment; will install + run
  when tools are available).

#### Known limitations

- `mold` not installed in this environment; `.cargo/config.toml` link config
  is commented out. Uncomment after `apt install mold`.
- `sccache` not installed; `rustc-wrapper` line is commented out. Uncomment
  after `cargo install sccache`.
- `cargo-deny`, `cargo-machete`, `cargo-bloat` not installed; config files
  are ready for when they are.

### 2026-08-19 — Subsystem spec set completed + Phase 1 dep wiring

Completed the subsystem spec coverage: every crate now has a matching spec.
Wired up real dependencies in 6 crate Cargo.tomls with versions resolved
against crates.io.

#### Added — new specs (5)

- `specs/core.toml` — canonical type spec: Message, Query, Response, Error,
  Context, Resource. Deep: `[types.*]`, `[api]`, `[error_model]`,
  `[lifecycle]`, `[threading]`. Absorbs canonical definitions from
  message.toml and query.toml (those are now subordinate).
- `specs/storage.toml` — helix-db adapter spec. Deep: `[implementation]`,
  `[api]`, `[error_model]`, `[cache_integration]`, `[lifecycle]`,
  `[threading]`.
- `specs/graphql.toml` — async-graphql projection spec. Deep:
  `[schema_generation]`, `[api]`, `[mapping]`, `[error_model]`,
  `[lifecycle]`, `[threading]`.
- `specs/mqtt.toml` — embassy MQTT transport spec. Deep: `[topics]`,
  `[api]`, `[mapping]`, `[error_model]`, `[lifecycle]`, `[threading]`.
- `specs/sse.toml` — SSE stream projection spec. Deep: `[streaming]`,
  `[api]`, `[mapping]`, `[error_model]`, `[lifecycle]`, `[threading]`.

#### Added — new crate

- `crates/themql-artifact/` — 19th crate. Model artifact validation and
  transfer between `themql-training` (desktop) and `themql-inference`
  (embedded). Cargo.toml wired with `serde` dep. Empty `src/lib.rs` stub.

#### Changed — specs (6)

- `specs/transport.toml` — trimmed. Removed per-transport detail blocks
  (`[mqtt]`, `[graphql]`, `[sse]`); they now live in the subordinate
  sub-specs. Kept `[principle]`, `[bridges]`, `[constraints]`. Added
  `[references] subspecs` pointer.
- `specs/message.toml` — rewritten as subordinate to `specs/core.toml`.
  Removed canonical Message type definition (moved to core.toml). Kept
  `[routing]`, `[serialization]`, `[identity]`, `[constraints]`. Added
  `subordinate_to` and `[references] canonical`.
- `specs/query.toml` — rewritten as subordinate to `specs/core.toml`.
  Removed canonical Query model (moved to core.toml) and transport
  projection blocks (moved to sub-specs). Kept `[resolution]`, `[cache]`,
  `[constraints]`. Added `subordinate_to` and `[references] canonical`.
- `specs/model_artifact.toml` — expanded to deep. Added
  `crate = "themql-artifact"`. New sections: `[api]`, `[validation_pipeline]`,
  `[error_model]`, `[lifecycle]`, `[threading]`, expanded `[constraints]`.
- `specs/training.toml` — `[artifact]` section now references
  `crate = "themql-artifact"`.
- `specs/inference.toml` — `[model_artifact]` section now references
  `crate = "themql-artifact"`.

#### Changed — workspace config

- `Cargo.toml` (root) — added `themql-artifact` to `[workspace] members`
  (19 total). Added `themql-artifact` path dep to `[workspace.dependencies]`.
  Resolved all workspace dep versions against crates.io:
  - `tch-rs` → `tch` (actual crate name)
  - `helixdb` → `helix-db` (actual crate name)
  - `async-graphql` → `8.0.0-rc.5`
  - `embassy-executor` → `0.10`, `embassy-sync` → `0.8`
  - `valkey` → `0.0.0-alpha5` (alpha)
  - `cachelito` → `0.16`, `moka` → `0.12`, `helix-db` → `3`
  - `polars` → `0.55`, `tch` → `0.24`, `ndarray` → `0.17`
  - `clap` → `4`, `ratatui` → `0.30`
- `SPEC.toml` — added `artifact = "themql-artifact"` to `[workspace.crates]`
  (19 total).

#### Changed — crate Cargo.tomls wired with real deps (7)

- `crates/themql-core/Cargo.toml` — `serde`, `serde_json`.
- `crates/themql-storage/Cargo.toml` — `helix-db`, `serde`.
- `crates/themql-graphql/Cargo.toml` — `async-graphql`, `themql-core`,
  `themql-query`.
- `crates/themql-mqtt/Cargo.toml` — `embassy-executor`, `embassy-sync`,
  `themql-core`, `themql-message`.
- `crates/themql-sse/Cargo.toml` — `tokio`, `themql-core`, `themql-message`.
- `crates/themql-artifact/Cargo.toml` — `serde`.

All wired crates have explicit `[lib]` sections with `name` and `path`.

#### Validation

- 41/41 TOML files parse via `python3 tomllib`.
- `cargo metadata --no-deps --format-version 1` resolves with 19 members.
- `cargo check --workspace` passes for all 19 crates with real deps
  resolved and downloaded from crates.io.

#### Known limitations carried forward

- `valkey` is `0.0.0-alpha5` (alpha) — may need replacement if unstable.
- `cachelito` is a proc-macro for function caching — may not match the L1
  use case; reassess in Phase 1.
- No real Rust source beyond 0-line stubs — Phase 1 proper.

### 2026-08-19 — AGENTS.md

- Created `AGENTS.md` at repo root — OpenCode convention file. Compact
  restatement of: authority hierarchy, read-these-first prompts, living-docs
  rule, validation commands (cargo fmt/check/test/clippy + TOML parse +
  cargo metadata), non-obvious architectural constraints (the 9 things an
  agent would likely get wrong), dependency policy, workspace layout, repo
  state, forbidden patterns, final report format. Points at
  `prompts/SYSTEM.md` as the full coding-agent contract.

### 2026-08-19 — Living docs adopted

- Adopted the living-docs rule: after every turn, update README, SECURITY,
  SESSION, HANDOVER, CHANGELOG, BUGS, MEMORY, AGENTS_SYNC at the repo root.
- Created all 7 living docs with current state.

### 2026-08-19 — v0.1 specification drop

First real specification + workspace skeleton drop. Replaces the v0.0 inline
spec that previously lived in the README (retained only in git history).

#### Added — specifications

- `SPEC.toml` — v0.1 constitution.
- `specs/message.toml`, `query.toml`, `cache.toml`, `transport.toml`,
  `runtime.toml`, `telemetry.toml`, `state_estimation.toml`, `gnc.toml`,
  `model_artifact.toml`, `analysis.toml`, `desktop.toml`, `embedded.toml`.
- `specs/training.toml` (desktop model creation) and `specs/inference.toml`
  (embedded model execution) — split from the proposed single `ml.toml`.

#### Added — prompts

- `prompts/SYSTEM.md` — authority hierarchy, core architectural rule,
  dependency policy, semantic ownership.
- `prompts/ARCHITECT.md` — desktop/embedded architecture, GNC authority,
  state representation, numerical policy, theDAF policy, ML lifecycle, online
  adaptation, sensors, cache, concurrency.
- `prompts/IMPLEMENTER.md` — implementation procedure, forbidden patterns,
  validation, final report template.
- `prompts/REVIEWER.md` — non-adversarial review checklist.
- `prompts/RED_TEAM.md` — adversarial review, red flags, finding format.
- `prompts/TEST_ENGINEER.md` — testing policy derived from `SPEC.toml
  [quality]` / `[safety]` plus ML-specific tests.

#### Added — workspace skeleton

- `Cargo.toml` — workspace root, 18 members, `[workspace.package]`,
  `[workspace.dependencies]` with placeholder versions, self-referencing
  path deps for all 18 crates.
- `rust-toolchain.toml` — stable channel, no pin.
- `crates/themql-*` × 18 — each with a `Cargo.toml` stub referencing
  `workspace = true` for version/edition/license.
  - 16 library crates with empty `src/lib.rs`.
  - 2 binary crates (`themql-desktop`, `themql-embedded`) with
    `src/main.rs` containing `fn main() {}`.
- `Cargo.lock` generated by `cargo check`.

#### Changed

- `README.md` rewritten as a short pointer. The previous 785-line inline
  v0.0 spec is retained only in git history.

#### Architectural decision: training/inference split

The proposed single `themql-ai` crate and `specs/ml.toml` were replaced
before writing by splitting model creation from model execution:

- `themql-training` (desktop) — tch-rs training, fine tuning, pruning,
  sparsification, artifact generation.
- `themql-inference` (embedded) — tch-rs inference, PINN inference, sparse
  inference, Bayesian update, constrained online adaptation.
- `specs/model_artifact.toml` governs the validated transfer between the two.

`SPEC.toml [ml]` keeps a single top-level key with `[ml.training]`,
`[ml.inference]`, `[ml.authority]`, and `[ml.artifact]` sub-tables.

#### Validation

- 35/35 TOML files parse via `python3 tomllib`.
- `cargo metadata --no-deps --format-version 1` resolves.
- `cargo check --workspace` passes for all 18 crates.

#### Not in this drop

- No real Rust source beyond the two 1-line binary stubs — Phase 1.
- No CI guard script.
- No dependency versions resolved against crates.io — placeholders only.
- No `[profile]` overrides.