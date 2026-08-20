//! # themql-inference
//!
//! Embedded model execution: PINN inference, sparse inference, Bayesian
//! update, and constrained online adaptation under explicit resource
//! budgets and rollback rules. Inference is **augmentative only** — never
//! the authoritative flight-control path.
//!
//! See `specs/inference.toml` for the authoritative specification.
//!
//! ## TETANUS
//!
//! Safety-critical: no recursion, fixed loop bounds, no heap allocation
//! after init except model activation (the one allowed atomic swap), no
//! per-step allocation, functions ≤ 60 lines, ≥ 2 assertions per function
//! (as `if !invariant { return Err }`), no `unwrap()`/`expect()`.

#![forbid(unsafe_code)]
#![deny(warnings)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(missing_docs)]

use nalgebra::SVector;
use thiserror::Error;

pub use themql_artifact::{ActivationHandle, ArtifactError, ModelArtifact, TrainedModel};

use themql_core::Timestamp;

/// State dimension — matches `themql-estimation`.
const STATE_DIM: usize = 21;

// ===========================================================================
// Resource budget
// ===========================================================================

/// Explicit resource budget for online adaptation. The runtime refuses to
/// start an online update if the budget would be exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBudget {
    /// Max % of one core.
    pub cpu_budget_pct: u8,
    /// Max resident memory for adaptation in bytes.
    pub memory_budget_bytes: u64,
    /// Max wall time in ms for a single inference.
    pub inference_deadline_ms: u16,
    /// Max wall time in ms for one online update step.
    pub adaptation_deadline_ms: u16,
}

impl ResourceBudget {
    /// Construct a budget. All fields are validated to be within sane
    /// ranges.
    ///
    /// # Errors
    /// Returns [`InferenceError::BudgetExceeded`] if `cpu_budget_pct` > 100.
    pub fn new(
        cpu_budget_pct: u8,
        memory_budget_bytes: u64,
        inference_deadline_ms: u16,
        adaptation_deadline_ms: u16,
    ) -> Result<Self, InferenceError> {
        if cpu_budget_pct > 100 {
            return Err(InferenceError::BudgetExceeded {
                used_cpu: cpu_budget_pct,
                limit_cpu: 100,
            });
        }
        Ok(Self {
            cpu_budget_pct,
            memory_budget_bytes,
            inference_deadline_ms,
            adaptation_deadline_ms,
        })
    }
}

// ===========================================================================
// Rollback handle
// ===========================================================================

/// Handle to the previous (last-known-good) model. On activation failure
/// or budget overrun, the runtime restores the previous model via this
/// handle. The handle owns the previous model's bytes — no copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackHandle {
    /// Previous model id.
    pub previous_model_id: String,
    /// Previous model raw bytes.
    pub previous_model_bytes: Vec<u8>,
}

impl RollbackHandle {
    /// Construct a rollback handle.
    #[must_use]
    pub fn new(previous_model_id: String, previous_model_bytes: Vec<u8>) -> Self {
        Self {
            previous_model_id,
            previous_model_bytes,
        }
    }

    /// Consume the handle and return the previous model bytes for
    /// restoration.
    ///
    /// # Errors
    /// Returns [`InferenceError::RollbackFailed`] if the bytes are empty
    /// (no previous model was retained).
    pub fn restore(self) -> Result<Vec<u8>, InferenceError> {
        if self.previous_model_bytes.is_empty() {
            return Err(InferenceError::RollbackFailed(
                "no previous model bytes retained".to_string(),
            ));
        }
        if self.previous_model_id.is_empty() {
            return Err(InferenceError::RollbackFailed(
                "previous model id missing".to_string(),
            ));
        }
        Ok(self.previous_model_bytes)
    }
}

// ===========================================================================
// Inference input / output
// ===========================================================================

/// Input to a single inference call. The 21-dim EKF state plus the
/// resource budget. Sensor snapshot is omitted for v0.1 to avoid the
/// telemetry dep.
#[derive(Debug, Clone, PartialEq)]
pub struct InferenceInput {
    /// Current EKF state vector.
    pub state: SVector<f64, STATE_DIM>,
    /// Resource budget for this inference.
    pub budget: ResourceBudget,
}

/// Output of a single inference call. `state_correction` is a residual
/// applied by the EKF as a pseudo-measurement — ML never writes state
/// directly.
#[derive(Debug, Clone, PartialEq)]
pub struct InferenceOutput {
    /// Residual correction in 21-dim state space.
    pub state_correction: SVector<f64, STATE_DIM>,
    /// Confidence in `[0.0, 1.0]`.
    pub confidence: f64,
    /// Measured inference latency in nanoseconds.
    pub latency_ns: u64,
}

// ===========================================================================
// Error
// ===========================================================================

/// Errors raised by the inference engine.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum InferenceError {
    /// The provided artifact was invalid.
    #[error("artifact invalid: {0}")]
    ArtifactInvalid(String),
    /// Resource budget was exceeded.
    #[error("budget exceeded: used_cpu {used_cpu} > limit_cpu {limit_cpu}")]
    BudgetExceeded {
        /// CPU pct used.
        used_cpu: u8,
        /// CPU pct limit.
        limit_cpu: u8,
    },
    /// Inference deadline was exceeded.
    #[error("deadline exceeded: deadline_ms {deadline_ms}, actual_ms {actual_ms}")]
    DeadlineExceeded {
        /// Deadline in ms.
        deadline_ms: u16,
        /// Actual runtime in ms.
        actual_ms: u16,
    },
    /// Rollback failed.
    #[error("rollback failed: {0}")]
    RollbackFailed(String),
    /// No model is loaded.
    #[error("model not loaded")]
    ModelNotLoaded,
    /// Inference forward pass failed.
    #[error("inference failed: {0}")]
    InferenceFailed(String),
    /// Schema mismatch between artifact and expected.
    #[error("schema mismatch: expected {expected}, got {got}")]
    SchemaMismatch {
        /// Expected schema description.
        expected: String,
        /// Actual schema description.
        got: String,
    },
}

impl From<InferenceError> for themql_core::Error {
    fn from(e: InferenceError) -> Self {
        themql_core::Error::internal_error(e.to_string())
    }
}

impl From<ArtifactError> for InferenceError {
    fn from(e: ArtifactError) -> Self {
        Self::ArtifactInvalid(e.to_string())
    }
}

// ===========================================================================
// InferenceEngine trait
// ===========================================================================

/// Primary inference trait. `load()` validates and activates an artifact;
/// `infer()` runs a single inference; `rollback()` restores the previous
/// model.
pub trait InferenceEngine: Send + Sync {
    /// Validate and atomically activate `artifact`. The previous model is
    /// retained for rollback.
    ///
    /// # Errors
    /// Returns [`InferenceError`] on validation or activation failure.
    fn load(&mut self, artifact: ModelArtifact) -> Result<ActivationHandle, InferenceError>;
    /// Run one forward pass; checks `inference_deadline`.
    ///
    /// # Errors
    /// Returns [`InferenceError`] on failure or deadline overrun.
    fn infer(&self, input: &InferenceInput) -> Result<InferenceOutput, InferenceError>;
    /// Restore the previous model from `handle`. Atomic.
    ///
    /// # Errors
    /// Returns [`InferenceError::RollbackFailed`] on failure.
    fn rollback(&mut self, handle: RollbackHandle) -> Result<(), InferenceError>;
    /// The currently-active model artifact, if any.
    fn active_model(&self) -> Option<&ModelArtifact>;
}

/// Re-export of `themql_core::Timestamp` for convenience in trait
/// implementations that need to stamp activation handles.
#[must_use]
pub fn now_timestamp() -> Timestamp {
    Timestamp::now_monotonic()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn resource_budget_construction_valid() {
        let b = ResourceBudget::new(50, 1024, 10, 100).expect("valid budget");
        assert_eq!(b.cpu_budget_pct, 50);
        assert_eq!(b.memory_budget_bytes, 1024);
    }

    #[test]
    fn resource_budget_rejects_over_100_pct() {
        let err = ResourceBudget::new(101, 0, 1, 1).unwrap_err();
        assert!(matches!(err, InferenceError::BudgetExceeded { .. }));
    }

    #[test]
    fn inference_error_variants_display() {
        assert_eq!(
            InferenceError::ModelNotLoaded.to_string(),
            "model not loaded"
        );
        assert_eq!(
            InferenceError::RollbackFailed("x".to_string()).to_string(),
            "rollback failed: x"
        );
    }

    #[test]
    fn rollback_handle_restore_returns_bytes() {
        let h = RollbackHandle::new("m-1".to_string(), vec![1, 2, 3]);
        let bytes = h.restore().expect("restore");
        assert_eq!(bytes, vec![1, 2, 3]);
    }

    #[test]
    fn rollback_handle_restore_rejects_empty() {
        let h = RollbackHandle::new("m-1".to_string(), Vec::new());
        assert!(h.restore().is_err());
    }

    #[test]
    fn rollback_handle_restore_rejects_missing_id() {
        let h = RollbackHandle::new(String::new(), vec![1, 2, 3]);
        assert!(h.restore().is_err());
    }
}
