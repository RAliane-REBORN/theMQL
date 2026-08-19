# theMQL


1. Repository contract
```
theMQL/
├── SPEC.toml
├── Cargo.toml
├── rust-toolchain.toml
│
├── specs/
│   ├── architecture.toml
│   ├── message.toml
│   ├── query.toml
│   ├── cache.toml
│   ├── transport.toml
│   ├── storage.toml
│   ├── runtime.toml
│   ├── ai.toml
│   ├── embedded.toml
│   └── cli.toml
│
├── prompts/
│   ├── SYSTEM.md
│   ├── ARCHITECT.md
│   ├── IMPLEMENTER.md
│   ├── REVIEWER.md
│   ├── RED_TEAM.md
│   └── TEST_ENGINEER.md
│
└── crates/
    ├── themql-core/
    ├── themql-message/
    ├── themql-query/
    ├── themql-cache/
    ├── themql-runtime/
    ├── themql-transport/
    ├── themql-storage/
    ├── themql-ai/
    ├── themql-embedded/
    ├── themql-cli/
    └── themql-graphql/
```
The critical distinction:

core defines semantics. Everything else implements those semantics.

No subsystem gets to quietly redefine what a message, query, cache hit, resolver, or stream means.


---

2. Main specification

```
SPEC.toml

[project]
name = "theMQL"
version = "0.1.0"
status = "greenfield"
language = "rust"
license = "MIT"

description = """
The Message Query Language: a native Rust message-query runtime
combining asynchronous messaging, resource querying, caching,
streaming, distributed execution, embedded operation, and native AI.
"""

[architecture]
model = "message_query_runtime"
primary_primitive = "message"
secondary_primitive = "query"

design_principle = """
All external interfaces are projections of the same internal
message/query execution model.
"""

semantic_owner = "themql-core"

[principles]
native_first = true
zero_copy_when_safe = true
async_first = true
embedded_capable = true
distributed_capable = true
transport_agnostic = true
storage_agnostic = true
cache_aware = true
model_agnostic = true

[constraints]
python_runtime_dependency = false
python_abi_dependency = false
typescript_runtime_dependency = false
custom_cache_implementation = false
custom_graph_database = false
custom_message_queue = false
custom_graphql_protocol = false

[forbidden]
semantic_reimplementation_of_dependencies = true
unnecessary_abstraction_layers = true
cross_subsystem_type_duplication = true
implicit_global_state = true
transport_specific_business_logic = true
storage_specific_business_logic = true
cache_specific_business_logic = true

[dependencies]
message_queue = "apalis"
mqtt = "embassy"
graphql = "async-graphql"
parallelism = "rayon"
l1_cache = "cachelito"
l2_cache = "moka"
l3_cache = "valkey"
storage = "helixdb"
ai = "candle"
cli = "clap"
tui = "ratatui"

[interfaces]
graphql = true
mqtt = true
sse = true
cli = true
embedded = true

[execution]
default_runtime = "tokio"
embedded_runtime = "embassy"
parallel_runtime = "rayon"

[cache]
tiers = ["L1", "L2", "L3", "L4"]

[cache.L1]
implementation = "cachelito"
scope = "process"
latency_class = "ultra_low"
authority = "ephemeral"

[cache.L2]
implementation = "moka"
scope = "process"
latency_class = "low"
authority = "ephemeral"

[cache.L3]
implementation = "valkey"
scope = "distributed"
latency_class = "network"
authority = "ephemeral"

[cache.L4]
implementation = "helixdb"
scope = "authoritative"
latency_class = "persistent"
authority = "authoritative"

[message]
identity_required = true
metadata_required = true
routing_required = true
serialization_explicit = true
transport_independent = true

[query]
resource_oriented = true
cache_aware = true
resolver_driven = true
transport_independent = true

[streaming]
mqtt = true
sse = true
graphql_subscription = true

[ai]
native_rust = true
embedding_runtime = "candle"
inference_runtime = "candle"

[quality]
tests_required = true
integration_tests_required = true
property_tests_required = true
benchmark_required_for_hot_paths = true
unsafe_requires_justification = true
public_api_requires_documentation = true

[compatibility]
abi = "none"
wire_protocol = "explicit"
serialization = "explicit"
```
That file establishes the constitution.

The subsystem specs then define how individual organs obey the constitution.


---

3. Message subsystem

specs/message.toml
```
[subsystem]
name = "message"
crate = "themql-message"
purpose = "Canonical message representation and routing semantics"

[ownership]
semantic_owner = true
transport_owner = false
storage_owner = false

[message]
required_fields = [
    "id",
    "timestamp",
    "subject",
    "operation",
    "payload",
    "metadata"
]

[operations]
allowed = [
    "query",
    "command",
    "event",
    "stream",
    "response",
    "error"
]

[identity]
id = "UUID"
correlation_id = "optional"
causation_id = "optional"

[routing]
subject_based = true
hierarchical_subjects = true

[serialization]
default = "serde"
binary_supported = true
text_supported = true

[constraints]
transport_independent = true
clone_minimization = true
ownership_explicit = true
```

---

4. Query subsystem

specs/query.toml
```
[subsystem]
name = "query"
crate = "themql-query"
purpose = "Canonical resource query model"

[query]
resource = "required"
selection = "required"
arguments = "optional"
context = "required"
projection = "optional"

[semantics]
read_only = true
side_effects = false
cacheable = true

[resolution]
resolver_based = true
parallel_resolution = true
batch_resolution = true

[cache]
cache_key_deterministic = true
cache_policy_explicit = true
cache_invalidation_explicit = true

[graphql]
adapter = "async-graphql"
projection = "graphql_selection_set"

[mqtt]
query_transport = true
response_transport = true

[sse]
stream_projection = true
```
This is where GraphQL becomes a projection, not the underlying ontology.


---

5. Cache subsystem

specs/cache.toml
```
[subsystem]
name = "cache"
crate = "themql-cache"

[architecture]
tiers = ["L1", "L2", "L3", "L4"]
promotion = true
demotion = true
invalidation = true

[L1]
crate = "cachelito"
scope = "local"
authority = "ephemeral"

[L2]
crate = "moka"
scope = "local"
authority = "ephemeral"

[L3]
crate = "valkey"
scope = "distributed"
authority = "ephemeral"

[L4]
crate = "helixdb"
scope = "authoritative"
authority = "persistent"

[semantics]
cache_hit = "resolved_value"
cache_miss = "next_tier"
authoritative_miss = "resolver_or_error"

[constraints]
no_custom_cache_engine = true
no_duplicate_cache_semantics = true
serialization_explicit = true
ttl_explicit = true
invalidation_explicit = true
```
TheMQL should not reinvent caching.

It should orchestrate caches.

That distinction is worth putting in the constitution because otherwise six months from now you'll discover RayanCacheManagerUltra™ buried underneath themql-cache.


---

6. Transport subsystem

specs/transport.toml
```
[subsystem]
name = "transport"
crate = "themql-transport"

[principle]
transport_is_projection = true
transport_contains_business_logic = false

[mqtt]
implementation = "embassy"
enabled = true
message_model = "themql-message"

[graphql]
implementation = "async-graphql"
enabled = true
query_model = "themql-query"

[sse]
enabled = true
message_model = "themql-message"

[bridge]
mqtt_to_graphql = true
graphql_to_mqtt = true
mqtt_to_sse = true
graphql_to_sse = true

[constraints]
transport_specific_types_must_not_escape_adapter = true
```
That bridge section is important.

You explicitly want:

MQTT ⇄ theMQL ⇄ GraphQL
              ⇅
             SSE

not:

MQTT → weird GraphQL adapter
GraphQL → weird MQTT adapter
SSE → third weird adapter

One internal model.


---

7. Runtime subsystem
```
[subsystem]
name = "runtime"
crate = "themql-runtime"

[execution]
async = true
default = "tokio"
embedded = "embassy"

[parallelism]
library = "rayon"
usage = "cpu_bound_work"

[queue]
library = "apalis"
role = "background_jobs_and_async_message_processing"

[concurrency]
structured = true
cancellation = true
backpressure = true

[constraints]
blocking_work_must_not_execute_on_async_executor = true
cpu_bound_work_must_use_rayon_or_explicit_worker_pool = true
unbounded_channels_forbidden = true
```

---

8. AI subsystem
```
[subsystem]
name = "ai"
crate = "themql-ai"

[library]
name = "candle"

[capabilities]
embeddings = true
inference = true
tensor_operations = true

[architecture]
native_rust = true
external_python_runtime = false

[integration]
query_results_as_input = true
message_results_as_input = true
embedding_cacheable = true

[constraints]
ai_must_not_be_required_for_core_runtime = true
model_provider_must_be_replaceable = true
```
AI is an optional capability, not a dependency that infects the message runtime like some kind of software fungus.


---

9. Embedded subsystem
```
[subsystem]
name = "embedded"
crate = "themql-embedded"

[runtime]
library = "embassy"

[mqtt]
enabled = true

[constraints]
no_std_capable = true
allocation_minimized = true
blocking_operations_forbidden = true

[compatibility]
core_semantics_shared_with_host = true
message_model_shared_with_host = true
wire_format_shared_with_host = true
```
This is where the architecture starts getting genuinely interesting.

The same conceptual message should be able to originate from:

ESP32
     ↓
Embassy
     ↓
MQTT
     ↓
theMQL
     ↓
GraphQL
     ↓
browser

without the embedded side needing to understand the web side.


---

10. Coding-agent system prompt

The coding agent should receive something much more restrictive than "implement this feature."

You are implementing theMQL, a greenfield native Rust message-query runtime.

The repository specifications are authoritative.

Priority order:

1. SPEC.toml
2. specs/*.toml
3. existing Rust APIs
4. tests
5. dependency documentation
6. agent assumptions

Never reverse this order.

ARCHITECTURAL RULES

- Rust is the canonical implementation language.
- No Python runtime dependency.
- No Python ABI.
- No handwritten reimplementation of existing dependency functionality.
- No custom cache engine.
- No custom message queue.
- No custom GraphQL implementation.
- No custom graph database.
- Do not introduce an abstraction merely to hide an existing abstraction.
- Prefer composition of established crates.
- Preserve ownership semantics.
- Prefer zero-copy where it is demonstrably safe and beneficial.
- Do not use unsafe Rust unless required and justified.
- Transport adapters must not contain business logic.
- Cache adapters must not define query semantics.
- Storage adapters must not define message semantics.

IMPLEMENTATION PROCESS

Before changing code:

1. Identify the subsystem.
2. Read the relevant subsystem TOML.
3. Identify the owning semantic type.
4. Identify existing crate capabilities.
5. Check whether the requested behavior already exists in a dependency.
6. Design the smallest integration necessary.
7. Implement.
8. Test.
9. Run clippy.
10. Verify architectural constraints.

If a dependency already provides the required primitive:

USE IT.

Do not recreate it from documentation.

If the specification conflicts with an existing implementation:

STOP.

Do not silently reinterpret the specification.

If a requested feature requires violating an architectural constraint:

STOP AND REPORT THE CONFLICT.

Do not invent a workaround.

OUTPUT REQUIREMENTS

Every implementation must identify:

- files changed
- subsystem affected
- architectural invariant preserved
- tests added or modified
- dependency APIs relied upon
- unsafe code introduced, if any

The goal is not maximum code.

The goal is minimum code necessary to produce correct composition of native primitives.

That last sentence is probably the most important instruction in the entire agent prompt.


---

11. Red-team prompt

This should be a separate agent rather than asking the implementation agent to review itself, because apparently letting software grade its own homework remains a popular engineering strategy.

You are the theMQL architectural red-team reviewer.

Your job is to find violations of the specification.

Do not optimize for politeness.

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

PRIMARY QUESTION:

"Did the implementation compose existing primitives, or did the developer accidentally rebuild part of the ecosystem?"

RED FLAGS:

- custom cache algorithms
- custom queues
- custom GraphQL parsing
- duplicated message types
- transport-specific domain objects
- conversion chains
- unnecessary clone()
- unnecessary Arc<Mutex<T>>
- blocking calls in async contexts
- hidden global state
- implicit cache invalidation
- serialization without an explicit contract
- Python compatibility layers
- ABI layers
- speculative abstraction
- dependency wrappers with no semantic value

For every finding provide:

SEVERITY
SUBSYSTEM
VIOLATED_INVARIANT
EVIDENCE
FAILURE_MODE
RECOMMENDED_FIX

Do not rewrite the architecture during review.

Identify violations first.


---

12. Implementation agent prompt

Then individual tasks become tiny.

Implement the requested change against theMQL specification.

Before coding:

1. Read SPEC.toml.
2. Read the relevant subsystem specification.
3. Locate the canonical primitive.
4. Inspect the existing dependency API.
5. Determine whether the requested capability already exists.

Do not implement functionality that belongs to an existing dependency.

Do not introduce a new abstraction unless the specification requires one.

Implement the smallest correct composition.

Required validation:

- cargo fmt --check
- cargo check
- cargo test
- cargo clippy

If any architectural constraint would be violated, stop rather than creating a workaround.

At completion report:

SUBSYSTEM:
CHANGE:
FILES:
DEPENDENCIES USED:
INVARIANTS PRESERVED:
TESTS:
RISKS:

The development order I'd use

Not "build everything."

Build the semantic spine first:

Phase 0
├── SPEC.toml
├── subsystem specs
└── CI architectural checks

Phase 1
├── Message
├── Query
├── Response
├── Error
└── Context

Phase 2
├── Resolver
├── Cache abstraction
├── L1
├── L2
├── L3
└── L4

Phase 3
├── Runtime
├── Apalis
└── backpressure / cancellation

Phase 4
├── MQTT
├── GraphQL
├── MQTT ↔ GraphQL bridge
└── SSE

Phase 5
├── HelixDB integration
├── query planning
└── distributed execution

Phase 6
├── Candle
├── embeddings
└── inference

Phase 7
├── Embassy
├── no_std boundaries
└── embedded MQTT

Phase 8
├── clap
├── Ratatui
└── operational tooling

The important architectural invariant throughout all eight phases is:

theMQL core must remain unaware of whether a message came from MQTT, GraphQL, SSE, an Apalis worker, an ESP32, or a CLI.