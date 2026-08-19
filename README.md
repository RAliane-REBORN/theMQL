# theMQL

The Message Query Language — a native Rust message-query runtime for desktop,
distributed, telemetry, analysis, AI, GNC, and embedded systems.

TheMQL defines a common message and query execution model. External protocols
and applications are projections of that model rather than independent
semantic systems.

## Specifications are authoritative

The repository TOML specifications are the source of truth for architecture,
crate boundaries, dependencies, authority, and safety. The coding agent
prompt is the source of truth for how an implementation must proceed.

- `AGENTS.md` — OpenCode agent entry point (read this first if you are an
  agent; it points at everything else)
- `SPEC.toml` — top-level constitution (v0.1)
- `specs/*.toml` — per-subsystem specifications
- `TETANUS.md` — NASA JPL Power of Ten rules adapted for Rust
  (safety-critical code constraints)
- `prompts/SYSTEM.md` — coding-agent contract entry point
- `prompts/*.md` — role-specific agent prompts

## Repository layout

```
theMQL/
├── SPEC.toml
├── Cargo.toml
├── rust-toolchain.toml
├── deny.toml           # cargo-deny config (licenses, advisories, bans)
├── TETANUS.md          # NASA JPL Power of Ten rules for Rust
├── .cargo/config.toml  # mold + sccache build config (opt-in)
├── specs/              # 19 subsystem specifications
├── prompts/            # coding-agent prompts (six roles)
└── crates/             # 19 crates: 17 libs + 2 binaries
```

## Crates

19 crates: 17 libraries + 2 binaries.

| Crate | Domain | Role |
|---|---|---|
| `themql-core` | core | semantic owner — canonical Message, Query, Response, Error, Context, Resource types |
| `themql-message` | core | message routing + serialization (subordinate to core.toml) |
| `themql-query` | core | query resolution + caching (subordinate to core.toml) |
| `themql-runtime` | runtime | tokio (desktop) / embassy (embedded) execution, apalis queue, rayon parallelism |
| `themql-cache` | cache | L1 cachelito / L2 moka / L3 valkey / L4 helix-db tiered cache |
| `themql-storage` | storage | helix-db authoritative storage adapter |
| `themql-transport` | transport | cross-cutting bridges + constraints (sub-specs: mqtt, graphql, sse) |
| `themql-graphql` | transport | async-graphql projection of themql-query |
| `themql-mqtt` | transport | embassy MQTT transport |
| `themql-sse` | transport | server-sent events projection |
| `themql-telemetry` | telemetry | first-class telemetry message schema |
| `themql-analysis` | desktop | polars-based analytical data processing |
| `themql-training` | desktop | tch model training, artifact generation (desktop only) |
| `themql-inference` | embedded | tch inference, PINN inference, constrained online adaptation |
| `themql-artifact` | cross-cutting | model artifact validation + transfer (training → inference) |
| `themql-gnc` | embedded | guidance/navigation/control — PID, LQRI, hybrid (deterministic) |
| `themql-estimation` | embedded | EKF, Bayesian update/sampling, quaternion state representation |
| `themql-embedded` | embedded binary | embassy-based embedded target binary |
| `themql-desktop` | desktop binary | dioxus UI + graphql + training + analysis binary |

## Authority hierarchy

1. `SPEC.toml`
2. `specs/*.toml`
3. tests
4. existing public APIs
5. dependency documentation
6. implementation assumptions

Never silently override a specification.

## Living docs

The repository carries living docs at the root. They are working state, not
historical artifacts. After every turn, the agent updates them (see
`MEMORY.md` for the standing rule):

- `README.md` — project entry point (this file)
- `SECURITY.md` — security policy and threat model
- `SESSION.md` — current session state and in-flight work
- `HANDOVER.md` — handover notes for the next agent/session
- `CHANGELOG.md` — chronological record of changes
- `BUGS.md` — known bugs and unresolved issues
- `MEMORY.md` — persistent facts and standing rules
- `AGENTS_SYNC.md` — coordination log when subagents or multiple agents run

## Status

v0.1 specification drop. Workspace skeleton exists; crate `src/` lands in
Phase 1. The previous v0.0 inline spec that lived in this README has been
superseded by the TOML spec set and is retained only in git history.