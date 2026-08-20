# theMQL Coding Agent — Test Engineer

This file defines the testing policy. Every change must satisfy `SPEC.toml
[quality]` and `[safety]`, plus the per-change requirements in
`prompts/IMPLEMENTER.md`.

## General requirements

From `SPEC.toml [quality]`:

- `cargo fmt --check`
- `cargo check`
- `cargo test`
- `cargo clippy`
- documentation required for every public API
- benchmarks required for hot paths
- `unsafe` requires written justification

## Test layers

### Unit tests

Every public function with non-trivial logic gets a unit test in the same
crate, in a `#[cfg(test)] mod tests` block or `tests/` directory per crate
convention.

### Property tests

Where the spec defines an invariant (`cache_key = "deterministic"`,
`side_effect_free = true`, `subject_based = true`, etc.), add a property
test that asserts the invariant over generated inputs. Use the property
testing crate already established in the workspace; do not introduce a new
one without a specification reason.

### Integration tests

Cross-crate behavior (e.g. `themql-query` resolving through `themql-cache`
backed by `themql-storage`) is verified in the binary crates' `tests/`
directory or in a dedicated `tests/integration_*.rs` file.

### Boundary tests

For numerical code (EKF, controllers, Bayesian update, sensor fusion):

- zero-input behaviour
- saturating input behaviour
- NaN / inf propagation rules
- quaternion norm near zero
- covariance matrix positive semi-definite boundary

### Failure-path tests

For every `Result`-returning public API, assert at least one error path
returns the expected error variant, not `panic!`.

### Numerical sanity tests

For EKF / GNC / estimation:

- known-input expected-output regression values
- covariance growth under known noise
- quaternion renormalisation after many updates
- bias estimate convergence under synthetic drift

### Regression tests

Every fixed bug gets a regression test that fails before the fix and passes
after.

## ML-specific tests

ML changes (training or inference) additionally require:

- model shape validation — input tensor shape matches feature schema
- input schema validation — feature names, dtypes, ordering
- output schema validation — output shape and dtype match artifact metadata
- artifact compatibility tests — a model artifact produced by
  `themql-training` must load in `themql-inference` after passing the
  `specs/model_artifact.toml` validation gate, and must fail to load when
  any validation field is broken
- rollback test — a failed activation reverts to the previously retained
  model
- online-adaptation budget test — adaptation refuses to start without an
  explicit resource budget, and stops when the budget is exhausted

## Safety-critical changes

Safety-critical paths (GNC, estimation, sensor failure detection, controller
health monitoring, ML fallback) additionally require:

- deterministic test — same input produces same output across runs
- failure-path test — sensor failure is detected and reported
- degradation test — ML failure does not disable the deterministic controller
- regression test — the safety invariant asserted in `SPEC.toml [safety]`
  holds

## What is forbidden in tests

- Tests that assert "the code runs" without asserting observable behaviour.
- Tests that mock the system under test so thoroughly they only test the
  mock.
- `#[ignore]` on a test without a written reason and a tracking follow-up.
- Tests that depend on wall-clock time without injecting a clock.