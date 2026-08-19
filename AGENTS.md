# AGENTS.md

Instructions for OpenCode agents working in this repository. Compact by
design — every line answers "would an agent likely miss this without help?".

## Authority hierarchy (do not invert)

1. `SPEC.toml`
2. `specs/*.toml`
3. tests
4. existing public APIs
5. dependency documentation
6. implementation assumptions

If a requested change conflicts with a spec: **stop and report**, do not
work around it. If a spec is insufficient: stop, do not invent architecture.

## Read these before changing code

- `prompts/SYSTEM.md` — authority hierarchy, dependency policy, semantic
  ownership, crate boundaries. Entry point for the coding-agent contract.
- `prompts/ARCHITECT.md` — desktop vs embedded architecture, GNC authority,
  state representation, numerical policy, theDAF policy, ML lifecycle,
  cache, concurrency.
- `prompts/IMPLEMENTER.md` — the 12-step implementation procedure, forbidden
  patterns, validation, final-report template.

Role-specific prompts (`REVIEWER.md`, `RED_TEAM.md`, `TEST_ENGINEER.md`)
apply when reviewing or testing.

## Living-docs rule (after every turn)

After every turn, update all 8 living docs at the repo root or the turn is
not finished: `README.md`, `SECURITY.md`, `SESSION.md`, `HANDOVER.md`,
`CHANGELOG.md`, `BUGS.md`, `MEMORY.md`, `AGENTS_SYNC.md`. Standing rule and
persistent facts live in `MEMORY.md`.

If no subagents ran in the turn, `AGENTS_SYNC.md` gets a one-line
"no subagents this turn" entry with the date.

## Validation commands (run in this order, all must pass)

### Required for every change (SPEC.toml [quality])

```
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Required by `SPEC.toml [quality]` and `prompts/IMPLEMENTER.md`. Do not report
success without all four passing (where source exists; `cargo test` is
trivial until Phase 1 adds real `src/`).

### Safety-critical changes (TETANUS.md)

For changes to `themql-gnc`, `themql-estimation`, `themql-inference`,
`themql-artifact`, also run:

```
cargo deny check
cargo machete --workspace
cargo bloat --release --crates
cargo +nightly miri test   # if any unsafe code exists
```

These enforce the NASA JPL Power of Ten rules codified in `TETANUS.md`.
Install with: `cargo install cargo-deny cargo-machete cargo-bloat`.

### TOML parse sanity (run after editing any spec)

```
python3 -c "import tomllib,pathlib;[tomllib.loads(p.read_text()) for p in pathlib.Path('.').rglob('*.toml') if '.git' not in p.parts]"
```

No exit = success. 40+ TOML files in this repo; a spec edit that breaks
parsing is the most common silent failure.

### Workspace resolution

```
cargo metadata --no-deps --format-version 1 > /dev/null
```

Fails if any crate Cargo.toml is malformed or a workspace member has no
target (lib.rs / main.rs / `[[bin]]`).

## Non-obvious architectural constraints

Things an agent would likely get wrong without being told:

- **`themql-ai` does not exist.** Model creation is `themql-training`
  (desktop); model execution is `themql-inference` (embedded). The
  boundary between them is governed by `specs/model_artifact.toml` and
  enforced by the `themql-artifact` crate. Do not reintroduce a single ML
  crate.
- **`tch-rs` is forbidden in the GNC control loop.**
  `specs/gnc.toml: tch_rs_in_control_loop = false`. This is deliberate, not
  an oversight. Use `ndarray` for deterministic numerics, `tch-rs` only for
  ML in `themql-training` / `themql-inference`. Note: the crate is published
  as `tch`, not `tch-rs`.
- **Quaternion is the canonical attitude representation.** Euler angles may
  be generated for UI/logging/debugging only. Never as the internal
  control/state representation.
- **ML must not be the authoritative flight-control path.** Authority
  hierarchy: deterministic GNC → EKF → Bayesian → ML-assisted state
  correction → ML prediction/anomaly detection. Basic flight stability
  must work without ML.
- **Transport adapters contain no business logic.** MQTT/GraphQL/SSE are
  projections of the core message/query model, not independent semantic
  systems. Transport-specific types must not escape their adapter crate.
- **theDAF is integration-only.** Not a core dependency, not an embedded
  dependency. Used for legacy data access and migration reference only.
- **No Python runtime, no Python ABI, no TypeScript runtime.** Spec-forbidden.
- **No custom cache/queue/GraphQL/database engine.** Compose the listed
  dependencies; do not reimplement them.
- **No `unsafe` without written justification.** `SPEC.toml [quality]
  unsafe_requires_justification = true`.

## Safety-critical code (TETANUS.md)

Changes to `themql-gnc`, `themql-estimation`, `themql-inference`,
`themql-artifact` must comply with the NASA JPL Power of Ten rules
codified in `TETANUS.md`. Summary:

1. No recursion in safety-critical crates.
2. All loops must have a fixed, statically provable upper bound.
3. No dynamic memory allocation after initialization (pre-allocate at init).
4. Functions ≤ 60 lines.
5. ≥ 2 assertions per function; assertion failure returns `Err`, never `panic!`.
6. Smallest possible scope for data objects.
7. Check all `Result` return values — no `unwrap()` / `expect()` in
   safety-critical code.
8. Simple macros only; no recursive `macro_rules!` munchers; minimal `cfg`.
9. One level of pointer dereference; no `**T`; function pointers discouraged.
10. Zero warnings at `clippy::pedantic` + `deny(warnings)`; pass
    `cargo deny`, `cargo machete`, `cargo bloat`, `cargo miri`.

See `TETANUS.md` for the full rationale and enforcement mechanisms.

## Dependency policy (use these, do not rebuild them)

async-graphql · embassy · apalis · rayon · cachelito (L1) · moka (L2) ·
valkey (L3) · helix-db (L4 + storage) · dioxus · polars · tch · ndarray ·
clap · ratatui

Workspace dep versions in `Cargo.toml [workspace.dependencies]` are
**resolved against crates.io** (2026-08-19). Prefer
`dep.workspace = true` in crate Cargo.tomls over repeating versions.

Note: `tch` is the crate name (published as `tch`, not `tch-rs`). `helix-db`
is the crate name (not `helixdb`). `valkey` is alpha
(`0.0.0-alpha5`); `cachelito` is a proc-macro for function caching — both
may need reassessment in Phase 1.

## Workspace layout (19 crates, 2 binaries)

17 library crates + 2 binary crates, all under `crates/`:

- **Core** (`themql-core`, `themql-message`, `themql-query`) — semantic
  owners. `themql-core` is canonical for Message/Query/Response/Error/
  Context/Resource types; `themql-message` and `themql-query` are
  subordinate (routing, resolution, caching specialisation).
- **Desktop-only** — `themql-desktop` (binary), `themql-analysis`,
  `themql-training`. Must not pull in embedded concerns.
- **Embedded-only** — `themql-embedded` (binary), `themql-inference`,
  `themql-gnc`, `themql-estimation`. Must not depend on dioxus, polars,
  helix-db, valkey, theDAF, or desktop training infrastructure.
- **Cross-cutting** — `themql-runtime`, `themql-cache`, `themql-storage`,
  `themql-transport`, `themql-graphql`, `themql-mqtt`, `themql-sse`,
  `themql-telemetry`, `themql-artifact` (model artifact validation +
  transfer between training and inference).

Cross-crate deps use `workspace = true` path deps declared in the root
`Cargo.toml [workspace.dependencies]` (e.g. `themql-message.workspace = true`).

## Repo state (v0.1, 2026-08-19)

- Spec + workspace skeleton only. No real Rust source beyond two 1-line
  binary `fn main(){}` stubs in `themql-desktop` / `themql-embedded`.
- All 19 crate Cargo.tomls exist; 7 have real deps wired (core, storage,
  graphql, mqtt, sse, artifact, + internal cross-crate refs).
- `cargo check --workspace` passes; `cargo test` is trivial until Phase 1
  adds real `src/`.
- No CI, no `opencode.json`, no pre-commit hooks. Validation is the agent's
  responsibility per the commands above.
- Changes are not committed unless the user explicitly asks.

## Forbidden implementation patterns (from prompts/IMPLEMENTER.md)

Python ABI bridges · custom GraphQL parsers · custom message queues ·
custom cache engines · unnecessary wrapper crates · duplicated Message
types · duplicated telemetry schemas · transport-specific domain models ·
implicit global state · uncontrolled `Arc<Mutex<_>>` · unnecessary
`clone()` · blocking operations in async paths · Euler-angle canonical
state · ML-controlled fallback-free flight paths · unvalidated model
activation · speculative abstractions · training code in `themql-inference`
· inference code in `themql-training`.

## Final report format (after implementing a change)

```
SUBSYSTEM:
CHANGE:
FILES_CHANGED:
DEPENDENCIES_USED:
SEMANTIC_OWNER:
ARCHITECTURAL_INVARIANTS:
TESTS:
VALIDATION:
UNSAFE_CODE:
RISKS:
FOLLOW-UP_REQUIRED:
```

Template from `prompts/IMPLEMENTER.md`. Do not report success unless the
required validation has actually passed.