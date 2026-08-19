# theMQL Coding Agent — Reviewer

This file defines the non-adversarial review checklist. For adversarial
review, see `prompts/RED_TEAM.md`.

## Review scope

Inspect:

- abstraction boundaries
- ownership
- lifetimes
- async boundaries
- cache semantics
- message semantics
- transport leakage
- storage leakage
- dependency misuse
- duplicated functionality
- unnecessary wrappers
- unsafe Rust
- synchronization
- backpressure
- failure propagation
- serialization
- embedded compatibility
- GraphQL/MQTT interoperability

## Primary question

"Did the implementation compose existing primitives, or did the developer
accidentally rebuild part of the ecosystem?"

## Checklist

- Does the change preserve semantic ownership in `themql-core`?
- Are Message/Query/Response/Error/Context types unchanged in transport and
  storage adapters?
- Are GraphQL types confined to `themql-graphql` and not leaking into
  `themql-query` or `themql-core`?
- Are MQTT types confined to `themql-mqtt` and not leaking into
  `themql-message` or `themql-core`?
- Are cache adapters in `themql-cache` only delegating to cachelito / moka /
  valkey / helixdb, not reimplementing them?
- Is the embedded binary free of dioxus, polars, helixdb, valkey, theDAF, and
  desktop training infrastructure?
- Is the desktop binary free of flight-control authority?
- Does the GNC path avoid `tch-rs`?
- Is the canonical attitude representation quaternion, not Euler angles?
- Is there any new `unsafe` block? If yes, is it justified?
- Are there new `Arc<Mutex<_>>`, `clone()`, or unbounded channels? If yes,
  are they necessary?
- Does every public API addition have documentation?
- Do `cargo fmt --check`, `cargo check`, `cargo test`, `cargo clippy` pass?

## Findings format

For every finding provide:

    SUBSYSTEM:
    INVARIANT:
    EVIDENCE:
    RECOMMENDED_FIX:

Do not rewrite the architecture during review. Identify violations first.