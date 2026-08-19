# HANDOVER.md

Handover notes for the next agent/session. Fold in-flight items from
`SESSION.md` here when a session ends. Update after every turn (see
`MEMORY.md` standing rules).

## Handover from: opencode (glm-5.2:cloud), 2026-08-19

### Repository state at handover

- v0.1 specification drop + workspace skeleton + living-docs + AGENTS.md +
  subsystem spec set + Phase 1 dep wiring + toolchain hardening all
  complete and validated.
- 19 crates, 43 TOML files, all parsing.
- `cargo check --workspace` passes for all 19 crates with real deps.
- Build toolchain config in place: rust-toolchain.toml (components + targets),
  .cargo/config.toml (mold/sccache opt-in), deny.toml (cargo-deny),
  TETANUS.md (Power of 10).
- Working tree has uncommitted changes. The user has not requested commits.

### What is done

- `SPEC.toml` (v0.1) with the training/inference split applied (19 crates).
- 19 `specs/*.toml` subsystem specs (every crate has a matching spec).
  - `core.toml` is canonical for Message/Query/Response/Error/Context/Resource.
  - `message.toml` and `query.toml` are subordinate to `core.toml`.
  - `transport.toml` is cross-cutting; `mqtt.toml`, `graphql.toml`, `sse.toml`
    are subordinate sub-specs.
  - `model_artifact.toml` is deep, with `crate = "themql-artifact"`.
- 6 `prompts/*.md` role files (SYSTEM, ARCHITECT, IMPLEMENTER, REVIEWER,
  RED_TEAM, TEST_ENGINEER).
- `Cargo.toml` workspace root with resolved dep versions + `[profile.dev]`,
  `[profile.release]`, `[profile.bench]` + `rust-toolchain.toml` (stable
  channel, components: rustfmt, clippy, rust-src, miri; targets: x86_64,
  thumbv7em, riscv32imc).
- `.cargo/config.toml` — mold linker + sccache wrapper configs (opt-in via
  comments). Cargo aliases: chk, tst, fmtc, clip, deny, machete.
- `deny.toml` — cargo-deny config (licenses, advisories, bans: pyo3/cpython/
  python3-sys/rusqlite, sources: crates.io only).
- `TETANUS.md` — NASA JPL Power of Ten adapted for Rust. 10 rules with
  enforcement mechanisms. Safety-critical crates identified: themql-gnc,
  themql-estimation, themql-inference, themql-artifact.
- 19 crate stubs (17 libs + 2 binaries). 7 crates have real deps wired:
  themql-core (serde, serde_json), themql-storage (helix-db, serde),
  themql-graphql (async-graphql, themql-core, themql-query), themql-mqtt
  (embassy-executor, embassy-sync, themql-core, themql-message), themql-sse
  (tokio, themql-core, themql-message), themql-artifact (serde).
- 8 living docs at repo root (MEMORY, CHANGELOG, SESSION, HANDOVER,
  SECURITY, BUGS, AGENTS_SYNC, README).
- `AGENTS.md` — OpenCode convention file with safety-critical validation
  section and TETANUS.md summary.
- Validation: 43/43 TOML parse, `cargo check --workspace` passes for all 19
  crates with real deps from crates.io.

### What is not done

- Real Rust source in `src/` — Phase 1 proper. All `src/lib.rs` and
  `src/main.rs` files are 0-line stubs.
- CI guard script.
- `opencode.json` — not yet written.
- `mold`, `sccache`, `cargo-deny`, `cargo-machete`, `cargo-bloat` not
  installed in this environment. Config files are ready for when they are.
  Install: `apt install mold`, `cargo install sccache cargo-deny cargo-machete
  cargo-bloat`. Then uncomment the relevant lines in `.cargo/config.toml`.
- Reassessment of `cachelito` (proc-macro for function caching — may not
  match L1 use case) and `valkey` (alpha version).

### Standing rules the next agent MUST follow

1. Authority hierarchy: `SPEC.toml` → `specs/*.toml` → tests → existing
   public APIs → dependency docs → assumptions. Never silently override.
2. Composition over reimplementation. Use the listed deps; do not rebuild
   them.
3. `themql-ai` does not exist. Model creation is `themql-training` (desktop);
   model execution is `themql-inference` (embedded). Artifact validation is
   `themql-artifact`. Boundary: `specs/model_artifact.toml`.
4. GNC authority: basic flight stability must work without ML. ML may
   augment the state estimate; it must not silently replace the deterministic
   controller.
5. Quaternion is canonical attitude. Euler angles only for UI/logging.
6. `tch` (published as `tch`, not `tch-rs`) is forbidden in the GNC control
   loop (`specs/gnc.toml: tch_rs_in_control_loop = false`).
7. theDAF is integration-only. Not a core dep. Not an embedded dep.
8. Living-docs rule: after every turn, update README, SECURITY, SESSION,
   HANDOVER, CHANGELOG, BUGS, MEMORY, AGENTS_SYNC. A turn is not finished
   while any of these is stale.
9. Do not commit unless the user explicitly asks.

### Suggested next actions (confirm with user first)

1. Decide commit strategy.
2. Begin Phase 1: real `src/lib.rs` content in `themql-core` (Message, Query,
   Response, Error, Context types) with tests.
3. Add a CI guard script (TOML parse + crate-name-vs-workspace invariant).
4. Reassess `cachelito` vs L1 use case.
5. Reassess `valkey` alpha stability.
6. (optional) Write `opencode.json`.

### Open questions / blockers

None. Awaiting user direction.

### Validation commands the next agent should run

- TOML parse: `python3 -c "import tomllib, pathlib; [tomllib.loads(p.read_text()) for p in pathlib.Path('.').rglob('*.toml') if '.git' not in p.parts]"`
- Workspace check: `cargo metadata --no-deps --format-version 1 > /dev/null`
- Full check: `cargo check --workspace`
- Once real source exists: `cargo fmt --check`, `cargo clippy --workspace`,
  `cargo test --workspace`