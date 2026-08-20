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

(none)

## Known limitations (not bugs, but tracked alongside)

These are spec-acknowledged gaps / v0.1 placeholders, not bugs. Phase 1 is
complete: all 19 crates have real `src/` content (traits + types + error
types + unit tests), 184 tests pass, full validation green.

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
- **`themql-inference` no tch-backed impl** — the `InferenceEngine` trait
  is defined but has no concrete tch-backed implementation; `tch` was
  deliberately not added to avoid the libtorch build dependency in this
  environment. A tch impl is a future task. Tracked as a limitation, not a
  bug.
- **`themql-runtime` EmbassyRuntime sleep stub** — the embedded runtime's
  sleep is a stub; embassy 0.10 `Spawner` is not `Send`/`Sync` so the
  `EmbeddedRuntime` trait was relaxed from the spec. Confirm before
  relying on the embedded runtime in flight. Tracked as a limitation.

### Cross-cutting follow-ups (not bugs)

- **CacheKey duplication** — `CacheKey` is defined locally in
  `themql-cache` (32-byte BLAKE3). `themql-query` also defines a
  `CacheKey` + `CacheKeyer` trait. The two key types must be reconciled
  — do not duplicate the key type silently. Tracked as a follow-up.
- **`serde-big-array` dependency** — added to workspace.dependencies for
  serializing the 21x21 covariance `[f64; 441]` in themql-telemetry.
  Reassess if a more idiomatic serde path emerges.
- **Local type stubs in dependent crates** — `themql-training` /
  `themql-analysis` define `FeatureSchema`/`ModelFormat`/
  `ValidationMetrics`/`Dataset` locally because the canonical owners
  (`themql-artifact`, `themql-telemetry`) now exist; switch the local
  definitions to re-exports. Tracked as a follow-up.

### Deferred heavy deps (not bugs)

- `themql-training`/`themql-analysis`/`themql-desktop`/`themql-embedded`
  defer heavy deps (tch, polars, dioxus, ratatui, embassy) — traits +
  minimal types only per the Phase 1 task brief. Concrete impls are future
  tasks.

### No concrete backends wired (not bugs)

- `themql-cache`, `themql-transport`, `themql-storage` define only the
  trait + type + error surface (per their specs). No concrete backends are
  wired yet: cachelito (L1), moka (L2), valkey (L3), helix-db (L4 storage),
  and the MQTT/GraphQL/SSE transport adapters are all unimplemented. The
  traits are ready for implementations to compose behind.
- `helix-db` v3.0.0 compiles cleanly in this environment and is listed as
  a dep of `themql-storage`, but is intentionally not imported in `lib.rs`
  — the `Storage` trait is backend-agnostic and the L4 adapter lands in a
  later phase.

### Tooling / CI (not bugs)

- No CI guard script yet. The architectural invariant "crate names in
  specs match workspace members" is currently enforced only by manual
  review.
- No `opencode.json`. The OpenCode-specific config file is not yet written;
  `AGENTS.md` exists as the agent convention file.
- `mold`, `sccache`, `cargo-deny`, `cargo-machete`, `cargo-bloat` not
  installed in this environment. Config files (`deny.toml`,
  `.cargo/config.toml`, `TETANUS.md`) are ready for when they are.
  Install: `apt install mold`, `cargo install sccache cargo-deny
  cargo-machete cargo-bloat`. Then uncomment the relevant lines in
  `.cargo/config.toml`.
- No fuzzing harness, no dependency-audit pipeline, no secret-management
  policy. Tracked in `SECURITY.md` known limitations.

### Supply chain (not bugs)

- `[workspace.dependencies]` versions are resolved against crates.io
  (2026-08-19), but `valkey` is alpha (`0.0.0-alpha5`) and `cachelito` is a
  proc-macro for function caching — both may need reassessment for the
  L1/L3 use cases.
