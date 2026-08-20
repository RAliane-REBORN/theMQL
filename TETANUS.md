# TETANUS.md

The Power of Ten — Rules for Developing Safety-Critical Code
adapted for Rust and theMQL.

Source: Gerard J. Holzmann, NASA/JPL Laboratory for Reliable Software.
Original paper targets C; this document adapts each rule to Rust's
semantics, noting where Rust's ownership model already satisfies a rule
and where additional discipline is still required.

`SPEC.toml [safety] safety_critical_paths_explicit = true` and
`prompts/RED_TEAM.md` enforce these rules. A safety-critical change that
violates any rule below must stop and report, not work around it.

## Rule 1 — Simple control flow: no goto, no recursion

> Restrict all code to very simple control flow constructs — no goto,
> setjmp/longjmp, no direct or indirect recursion.

### Rust adaptation

- **`goto` / `setjmp` / `longjmp`** — do not exist in safe Rust. `unsafe`
  blocks that invoke them via FFI are forbidden in safety-critical paths.

- **Recursion** — forbidden in safety-critical crates (`themql-gnc`,
  `themql-estimation`, `themql-inference`, `themql-artifact`). All
  functions must be statically non-recursive. The acyclic call graph this
  guarantees enables bounded-execution proofs.

- **Loops** — `loop`, `while`, `while let`, `for` are allowed only with a
  fixed upper bound (see Rule 2). `loop` without a `break` with a
  provable exit condition is forbidden in safety-critical code.

### Enforcement

- Clippy: `clippy::recursion` (nightly) flags direct recursion.
- Manual review: indirect recursion (A calls B calls A) must be
  flagged by the red-team reviewer.
- Spec: `specs/gnc.toml [constraints]`, `specs/state_estimation.toml
  [constraints]`.

## Rule 2 — All loops must have a fixed upper bound

> It must be trivially possible for a checking tool to prove statically
> that a preset upper-bound on the number of iterations of a loop cannot
> be exceeded.

### Rust adaptation

- Every `for` loop over a slice/iterator must have a provable maximum
  element count.
- Every `while` / `while let` loop must have an explicit iteration
  counter with a compile-time maximum, or a bound derivable from the
  input type's capacity.
- `loop { ... }` is forbidden in safety-critical code unless it is a
  non-terminating scheduler (embedded main loop) and explicitly marked
  as such with a spec reference.

### Enforcement

- Pattern: `for i in 0..MAX_ITERATIONS { ... }` is preferred over
  `while condition { ... }`.
- If a bound cannot be proven statically, add an explicit counter and
  `return Err(...)` when it exceeds the limit.
- Spec: `specs/gnc.toml [constraints]`, `specs/state_estimation.toml
  [constraints]`.

## Rule 3 — No dynamic memory allocation after initialization

> Do not use dynamic memory allocation after initialization.

### Rust adaptation

- **Desktop crates** (`themql-desktop`, `themql-analysis`,
  `themql-training`) — heap allocation is permitted; these are not
  safety-critical.

- **Embedded safety-critical crates** (`themql-gnc`,
  `themql-estimation`, `themql-inference`, `themql-artifact`) — no
  `Vec::new()`, `Box::new()`, `String::new()`, `HashMap::new()`, or any
  other heap-allocating call after `init`. Pre-allocate all buffers
  at startup; pass them by reference.

- **`no_std`** — `themql-embedded`, `themql-gnc`,
  `themql-estimation` should be `no_std` or `no_std + alloc` with
  allocation restricted to init.

### Enforcement

- `cargo bloat --crates` to find unexpected allocators.
- `cargo nm --release | grep 'alloc'` to check for allocator symbols in
  embedded builds.
- Clippy: `clippy::vec_box`, `clippy::boxed_local`.
- Spec: `specs/embedded.toml [runtime] allocation = "minimized"`.

## Rule 4 — Functions no longer than ~60 lines

> No function should be longer than what can be printed on a single sheet
> of paper in a standard reference format with one line per statement and
> one line per declaration. Typically, no more than about 60 lines.

### Rust adaptation

- 60 lines max per function, including signatures and doc comments.
- This is a soft limit enforced by review; a function at 65 lines with
  a clear logical unit is acceptable. A function at 100 lines is not.
- Extract sub-functions when logic exceeds the limit.

### Enforcement

- Manual review.
- Optional: `clippy::too_many_lines` (nightly) with threshold.
- Spec: `SPEC.toml [quality] documentation_required_for_public_api =
  true` (forces function-level doc comments which count toward the
  limit, encouraging decomposition).

## Rule 5 — Assertion density: minimum 2 per function

> The assertion density of the code should average to a minimum of two
> assertions per function. Assertions must be side-effect free and defined
> as Boolean tests. When an assertion fails, an explicit recovery action
> must be taken.

### Rust adaptation

- Use `assert!`, `debug_assert!`, `const_assert!` for invariants.
- **Safety-critical crates** — use `assert!` (not `debug_assert!` —
  assertions must fire in release builds).
- **Non-critical crates** — `debug_assert!` is acceptable.
- Every assertion failure must return an `Err(...)`, not `panic!`.
  In safety-critical code: `if !condition { return Err(Error::...); }`
  is the preferred form.
- Assertions must be side-effect free — no function calls inside the
  assertion that mutate state.

### Enforcement

- Manual review counts assertions per function.
- Clippy: `clippy::assertions_on_constants` flags useless assertions.
- Spec: `SPEC.toml [safety]` + `prompts/TEST_ENGINEER.md` boundary
  tests.

## Rule 6 — Smallest possible scope for data objects

> Data objects must be declared at the smallest possible level of scope.

### Rust adaptation

- Prefer local bindings inside blocks over struct fields.
- Prefer `let` inside `if`/`for`/`match` arms over function-level `let`.
- Do not hoist variables to function scope if they are only used in one
  branch.
- No `mut` at a broader scope than necessary.

### Enforcement

- Clippy: `clippy::needless_late_init`, `clippy::needless_mut`.
- Manual review.

## Rule 7 — Check all return values and validate all parameters

> The return value of non-void functions must be checked by each calling
> function, and the validity of parameters must be checked inside each
  function.

### Rust adaptation

- **Return values** — Rust's `Result` type makes this near-automatic:
  `?` propagates errors. Never call `.unwrap()` or `.expect()` on a
  `Result` in safety-critical code. Use `?` or explicit `match`.

- **Parameter validation** — every public function in a safety-critical
  crate must validate its parameters on entry:
  - Numeric ranges (e.g. `index < len`, `value >= 0.0`).
  - Non-null pointers (in `unsafe` code only — safe Rust handles this).
  - Quaternion norm ≈ 1.0 (for attitude representations).
  - Covariance matrix positive semi-definite (for EKF).

- **Ignoring a return value** — `let _ = function()` is acceptable only
  with a comment explaining why. Bare `function();` discarding a
  `Result` is forbidden in safety-critical code.

### Enforcement

- Clippy: `clippy::unwrap_used`, `clippy::expect_used` (in CI for
  safety-critical crates).
- Clippy: `clippy::result_unit_err`, `clippy::unused_result`.
- Manual review for parameter validation completeness.
- Spec: `prompts/RED_TEAM.md` flags `unwrap()` in safety-critical paths.

## Rule 8 — Restricted preprocessor: no token pasting, no recursive macros

> The use of the preprocessor must be limited to the inclusion of header
> files and simple macro definitions. Token pasting, variable argument
> lists, and recursive macro calls are not allowed.

### Rust adaptation

- **`include!`** — allowed (equivalent to `#include`).
- **`macro_rules!`** — allowed for simple, hygienic macros. Complex
  macros that obscure control flow are forbidden in safety-critical
  crates.
- **Declarative macros with `tt` munchers** (recursive macro patterns)
  — forbidden in safety-critical crates.
- **Procedural macros** — allowed only in `themql-artifact` (if needed
  for derive macros) and build scripts. Not in the GNC/estimation
  control loop.
- **`cfg` conditional compilation** — keep to a minimum. Each `cfg`
  attribute multiplies the test matrix. Prefer runtime configuration
  over compile-time branching where feasible.

### Enforcement

- Manual review of macro complexity.
- Clippy: `clippy::macro_use_imports` discourages `#[macro_use]`.
- Count `#[cfg(...)]` attributes per crate — flag if > 5.

## Rule 9 — Restricted pointers: no more than one level of dereferencing

> No more than one level of dereferencing is allowed. Function pointers
> are not permitted.

### Rust adaptation

- **Safe Rust** — references are the primary pointer type; `&T` and
  `&mut T` are one level. `&(&T)` (double reference) is rarely needed
  and should be avoided.
- **`unsafe` raw pointers** — `*const T`, `*mut T` — restricted in
  safety-critical crates. If used (e.g. for hardware register access),
  no `**T` (pointer-to-pointer).
- **Function pointers** — `fn(...)` types are allowed for dispatch
  tables but should be avoided in the control loop. Prefer `enum`
  dispatch or trait objects with `dyn` only where statically provable.
- **Smart pointers** — `Box<T>`, `Rc<T>`, `Arc<T>` are one level of
  indirection; allowed where appropriate, but `Box<Box<T>>` or
  `Arc<Arc<T>>` are forbidden.

### Enforcement

- Clippy: `clippy::ptr_ptr` flags double pointers.
- Clippy: `clippy::fn_to_numeric_numeric` flags function pointer casts.
- Manual review for `unsafe` blocks in safety-critical crates.
- Spec: `SPEC.toml [quality] unsafe_requires_justification = true`.

## Rule 10 — Compile with all warnings at the most pedantic setting; zero warnings

> All code must be compiled, from the first day of development, with all
> compiler warnings enabled at the compiler's most pedantic setting. All
> code must compile without any warnings. All code must be checked daily
> with at least one, but preferably more than one, state-of-the-art
> static source code analyzer and should pass with zero warnings.

### Rust adaptation

- **Rustc warnings** — `#![deny(warnings)]` or `-D warnings` in CI.
- **Clippy** — `cargo clippy --workspace --all-targets -- -D warnings`
  with a comprehensive `clippy::pedantic` lint group enabled at the
  workspace level.
- **cargo-deny** — `cargo deny check` for license/advisory/ban
  compliance (see `deny.toml`).
- **cargo-machete** — `cargo machete --workspace` to find unused
  dependencies (dead deps are a maintenance and supply-chain risk).
- **cargo-bloat** — `cargo bloat --release` to monitor binary size
  growth (relevant for embedded targets with limited flash).
- **Miri** — `cargo +nightly miri test` for UB detection in `unsafe`
  code. Required for any crate that contains `unsafe`.

### Enforcement — the validation gate

Run in this order, all must pass:

```
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
cargo machete --workspace
cargo bloat --release --crates   # embedded-target size monitoring
cargo +nightly miri test          # if any unsafe code exists
```

The first four are required by `SPEC.toml [quality]` and
`prompts/IMPLEMENTER.md`. The last four are required by TETANUS.md for
safety-critical crates.

### Workspace lint configuration

Each safety-critical crate (`themql-gnc`, `themql-estimation`,
`themql-inference`, `themql-artifact`) should have at the top of
`lib.rs`:

```rust
#![deny(warnings)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
```

Non-safety-critical crates may use `#![warn(clippy::pedantic)]` without
`#![deny(warnings)]`.

## Summary table

| # | Rule | Rust mechanism | Enforcement |
|---|------|---------------|-------------|
| 1 | Simple control flow, no recursion | No goto exists; ban recursion in safety-critical | Clippy + review |
| 2 | Fixed loop upper bound | Explicit counters, provable iteration limits | Clippy + review |
| 3 | No dynamic allocation after init | Pre-allocate at init, no heap in control loop | cargo bloat + review |
| 4 | Functions ≤ 60 lines | Extract sub-functions | Review |
| 5 | 2+ assertions per function | `assert!` / `if !cond { return Err }` | Review + tests |
| 6 | Smallest scope for data | `let` inside blocks, no premature hoisting | Clippy + review |
| 7 | Check all return values | `Result` + `?`, no `unwrap()`/`expect()` | Clippy + review |
| 8 | Restricted macros | Simple `macro_rules!` only, minimal `cfg` | Review |
| 9 | One level of pointer deref | `&T` / `&mut T`, no `**T`, careful `unsafe` | Clippy + review |
| 10 | Zero warnings, pedantic, static analyzers | `deny(warnings)`, clippy pedantic, cargo-deny, machete, bloat, miri | CI gate |

## Safety-critical crate identification

The following crates are safety-critical and must comply with ALL ten rules:

- `themql-gnc` — guidance/navigation/control
- `themql-estimation` — EKF, state estimation
- `themql-inference` — embedded ML inference
- `themql-artifact` — model artifact validation

The following crates are not safety-critical but should follow rules 4,
6, 7, 10 as general quality practice:

- `themql-core`, `themql-message`, `themql-query`
- `themql-runtime`, `themql-cache`, `themql-storage`
- `themql-transport`, `themql-graphql`, `themql-mqtt`, `themql-sse`
- `themql-telemetry`, `themql-analysis`, `themql-training`
- `themql-embedded` (binary), `themql-desktop` (binary)