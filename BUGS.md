# BUGS.md

Known bugs and unresolved issues. Empty when all known issues are resolved.
Update after every turn (see `MEMORY.md` standing rules).

## Format

One entry per bug. Use the template below. Close an entry with a `RESOLVED`
line + date + commit/PR reference when fixed.

```
### BUG-NNNN: <short title>

- Discovered: YYYY-MM-DD
- Severity: blocker | major | minor | cosmetic
- Subsystem: themql-<crate> | specs | prompts | docs | ci
- Status: open | in-progress | resolved
- Symptom: what is observed
- Expected: what the spec or correct behaviour requires
- Reproduction: minimal steps / command
- Root cause: (filled when known)
- Fix: (filled when known; include commit hash)
- Follow-up: regression test, spec clarification, etc.
```

## Open bugs

(none)

## Resolved bugs

### BUG-0007: themql-cache / themql-storage workspace compile breakage (pre-existing)

- Discovered: 2026-08-20 (during Phase 2 Stages 6-8 turn; pre-existing
  in the working tree before the turn began, confirmed via `git stash`)
- Severity: blocker (for `cargo check --workspace` only)
- Subsystem: themql-cache, themql-storage
- Status: resolved
- Symptom: `cargo check --workspace` fails with 3 errors in
  `themql-cache/src/lib.rs` around `.await` on a non-future
  (`self.promote(key, &entry_for_promote, CacheTier::L4).await`).
- Expected: `cargo check --workspace` passes for all 20 crates.
- Reproduction: `cargo check --workspace`.
- Root cause: uncommitted changes in `themql-cache/Cargo.toml`,
  `themql-cache/src/lib.rs`, `themql-storage/Cargo.toml`,
  `themql-storage/src/lib.rs` (1939 lines added across 5 files) that were
  in the working tree before the Phase 2 Stages 6-8 turn. NOT introduced
  by Stages 6-8 (which only touch themql-runtime, themql-desktop,
  themql-embedded).
- Fix: resolved 2026-08-20 (Phase 2 Stages 2-9 turn). The cache/storage
  changes were completed and integrated; `cargo check --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace` all pass clean (240 tests).
- Follow-up: none.

## Known limitations (not bugs, but tracked alongside)

These are spec-acknowledged gaps / v0.1 placeholders, not bugs. Phase 2
Stages 1-11 are complete: all 20 crates have real `src/` content,
cache/storage/transport backends wired, runtime impls + desktop/embedded
binary wiring, analysis/training/inference heavy-dep wiring, cross-crate
type reconciliation, and safety-critical tooling installed. 240 tests
pass workspace-wide (default features). Full validation green.

### Safety-critical placeholders (MUST address before deployment)

- **`themql-artifact` HashValidator placeholder hash** — uses a
  deterministic fold hash, NOT real SHA-256 or BLAKE3. NOT cryptographically
  secure. A malicious or corrupt model artifact could pass the integrity
  check. v0.1 placeholder to avoid a crypto crate dep in this
  safety-critical crate. MUST replace with a real cryptographic hash before
  deployment. Tracked as a limitation, not a bug.
- **`themql-estimation` simplified EKF** — the v0.1 `Ekf` uses
  identity-gain updates (predict advances time + adds process noise;
  updates apply simplified scalar-gain covariance shrinkage), NOT full
  nonlinear quaternion dynamics + Jacobian-based Kalman gain. State
  estimation is NOT flight-ready. Tracked as a limitation, not a bug.
- **`themql-runtime` EmbassyRuntime sleep stub** — the embedded runtime's
  sleep is a stub; embassy 0.10 `Spawner` is not `Send`/`Sync` so the
  `EmbeddedRuntime` trait was relaxed from the spec. Confirm before
  relying on the embedded runtime in flight. Tracked as a limitation.

### Environment constraints (not bugs)

- **`tch-backend` feature tests not run** — `themql-training` and
  `themql-inference` define `tch-backend` features wiring `tch` with
  `download-libtorch`. The feature compiles clean, but tests
  (`TchTrainer`, `TchInferenceEngine`) are not run in this environment
  due to libtorch download size + RAM constraints (7.8GB RAM, no swap;
  polars-core alone OOM-kills rustc without `CARGO_PROFILE_DEV_DEBUG=0`).
  The default-feature tests pass. Tracked as a limitation.

### Cross-cutting follow-ups (not bugs)

- **CacheKey duplication — RESOLVED** — `CacheKey` is now re-exported
  from `themql-query` in `themql-cache` per `specs/cache.toml`. The
  `themql-query` version was enriched with `Serialize`/`Deserialize`
  derives + `hash_of()` method.
- **Local type stubs — RESOLVED** — `themql-training` and
  `themql-artifact` now re-export shared schema types from
  `themql-schema` (20th crate). No more local duplicate definitions of
  `FeatureSchema`/`ModelFormat`/`ValidationMetrics`/`TrainedModel`.
- **`serde-big-array` dependency** — added to workspace.dependencies for
  serializing the 21x21 covariance `[f64; 441]` in themql-telemetry.
  Reassess if a more idiomatic serde path emerges.

### Stub backends (not bugs)

- `themql-cache` L1 (cachelito API mismatch — `HashMap` fallback used),
  L3 (valkey alpha driver lacks `DEL`/`EXPIRE`/binary — returns
  `TierUnavailable`), and `themql-storage` L4 (helix-db needs live server
  — in-memory `HashMap` fallback used). Real adapters are future tasks.
- `invalidate_pattern` in `TieredCache` is best-effort (no
  key→subject index yet — effectively a no-op for L1/L2).

### Tooling / CI (not bugs)

- `cargo-deny`, `cargo-machete`, `cargo-bloat` installed and passing.
  `cargo deny check` passes (BSL-1.0 + CDLA-Permissive-2.0 added to
  allowed licenses). `cargo machete --with-metadata` clean (11 unused
  deps removed). `cargo bloat` passes on desktop binary.
- `opencode.json` written with `$schema`, `instructions: ["AGENTS.md"]`,
  permission rules, and `validate` + `safety-gate` custom commands.
- `scripts/ci_guard.py` checks TOML parse + crate-name-vs-workspace
  invariant. All checks pass.
- `mold`, `sccache` not installed. Config files (`.cargo/config.toml`)
  are ready for when they are. Install: `apt install mold`,
  `cargo install sccache`. Then uncomment the relevant lines.
- No fuzzing harness, no dependency-audit pipeline, no secret-management
  policy. Tracked in `SECURITY.md` known limitations.

### Supply chain (not bugs)

- `[workspace.dependencies]` versions are resolved against crates.io
  (2026-08-19), but `valkey` is alpha (`0.0.0-alpha5`) and `cachelito` is a
  proc-macro for function caching — both may need reassessment for the
  L1/L3 use cases.
