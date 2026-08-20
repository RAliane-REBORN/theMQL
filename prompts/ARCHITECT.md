# theMQL Coding Agent — Architecture

This file defines the architectural rules that constrain every implementation
change. It is read after `prompts/SYSTEM.md`.

## Desktop architecture

The desktop binary is:

    themql-desktop

It provides:

- Dioxus UI
- GraphQL
- SSE
- MQTT
- Polars
- telemetry analysis
- theDAF integration
- tch-rs training
- PINN training
- gradient boosting
- fine tuning
- pruning
- model artifact generation
- HelixDB
- Valkey
- CLI/TUI

The desktop binary is the primary development,
analysis, training, telemetry, and operational environment.

Training lives in `themql-training` and is desktop-only.

## Embedded architecture

The embedded binary is:

    themql-embedded

It provides:

- Embassy
- MQTT
- telemetry acquisition
- sensor integration
- EKF
- Bayesian updating
- Bayesian sampling
- quaternion state representation
- PID
- LQRI
- hybrid LQRI-PID
- model inference
- sparse/PINN inference
- controlled online adaptation

Inference lives in `themql-inference`.

It must not depend on desktop infrastructure.

Do not introduce:

- Dioxus
- Polars
- HelixDB
- Valkey
- theDAF
- desktop training infrastructure

into the embedded binary.

## GNC authority rule

Basic vehicle stability must remain possible without machine learning.

The authority hierarchy is:

1. deterministic GNC
2. EKF/state estimation
3. Bayesian uncertainty/model reasoning
4. ML-assisted state correction
5. ML prediction/anomaly detection

ML must not silently become the authoritative flight-control path.

A model may augment the state estimate.

A model may not silently replace the deterministic controller.

## State representation

Quaternion attitude is canonical.

Do not use Euler angles as the internal attitude representation.

Euler angles may be generated for:

- UI
- logging
- debugging
- human-readable telemetry

They must not become the canonical control/state representation.

## Numerical policy

Use ndarray for:

- EKF numerical operations
- covariance matrices
- conventional linear algebra
- deterministic numerical algorithms

Use tch-rs for:

- neural networks
- PINNs
- tensor operations
- model training
- fine tuning
- pruning
- ML inference

Do not introduce tch-rs into deterministic GNC merely for convenience.

`specs/gnc.toml` enforces `tch_rs_in_control_loop = false`. That is deliberate.

## theDAF policy

theDAF is an integration component.

It is not the semantic owner of theMQL.

Use theDAF for:

- existing analysis functionality
- legacy data access
- telemetry analysis
- migration support
- comparison against the new implementation

Do not make theMQL core depend on theDAF.

Do not make the embedded binary depend on theDAF.

If functionality is migrated from theDAF into theMQL,
the new implementation must use native Rust primitives.

Do not reproduce the old ABI/Python architecture.

## ML model lifecycle

Models follow:

    telemetry
        ↓
    Polars
        ↓
    dataset
        ↓
    tch-rs training
        ↓
    validation
        ↓
    fine tuning
        ↓
    pruning
        ↓
    sparsification
        ↓
    model artifact
        ↓
    embedded validation
        ↓
    inference

A model must not be activated on the embedded target without:

- version validation
- schema validation
- compatibility validation
- artifact integrity validation

Previous valid models must remain recoverable.

The artifact boundary is governed by `specs/model_artifact.toml`.

## Online adaptation

Online learning is explicitly constrained.

Before implementing online training:

1. define the training budget
2. define memory limits
3. define computational limits
4. define convergence criteria
5. define model rollback
6. define activation criteria
7. define failure behaviour

Never allow an online model update to automatically become
flight-control authority.

## Sensor architecture

Canonical sensors:

- NEO-6M GPS
- BME280 barometer
- GY-LSM6DS3 IMU

Sensor failures must be detectable.

The estimator must represent uncertainty.

No sensor should be assumed permanently valid.

## Cache architecture

Use:

    L1 Cachelito
    L2 Moka
    L3 Valkey
    L4 HelixDB

L4 is authoritative.

Do not implement another cache.

Do not hide cache invalidation.

Cache keys must be deterministic.

TTL and invalidation policies must be explicit.

## Concurrency

Use async execution for I/O.

Use Rayon for CPU-bound parallel workloads.

Do not perform blocking CPU-heavy work directly on an async
executor.

Prefer bounded channels and explicit backpressure.

Cancellation must propagate through asynchronous workflows.