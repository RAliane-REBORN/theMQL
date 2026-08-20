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

pub use themql_artifact::{ActivationHandle, ArtifactError, ArtifactValidator, ModelArtifact};
pub use themql_schema::TrainedModel;

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
                used: ResourceBudget {
                    cpu_budget_pct,
                    memory_budget_bytes,
                    inference_deadline_ms,
                    adaptation_deadline_ms,
                },
                limit: ResourceBudget {
                    cpu_budget_pct: 100,
                    memory_budget_bytes,
                    inference_deadline_ms,
                    adaptation_deadline_ms,
                },
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
/// handle. The handle owns the previous model — no copy.
#[derive(Debug, Clone, PartialEq)]
pub struct RollbackHandle {
    /// Previous model id.
    pub previous_model_id: String,
    /// Previous model (opaque bytes + metadata).
    pub previous_model: TrainedModel,
}

impl RollbackHandle {
    /// Construct a rollback handle.
    #[must_use]
    pub fn new(previous_model_id: String, previous_model: TrainedModel) -> Self {
        Self {
            previous_model_id,
            previous_model,
        }
    }

    /// Consume the handle and return the previous model for restoration.
    ///
    /// # Errors
    /// Returns [`InferenceError::RollbackFailed`] if the model bytes are
    /// empty (no previous model was retained).
    pub fn restore(self) -> Result<TrainedModel, InferenceError> {
        if self.previous_model.model_bytes.is_empty() {
            return Err(InferenceError::RollbackFailed(
                "no previous model bytes retained".to_string(),
            ));
        }
        if self.previous_model_id.is_empty() {
            return Err(InferenceError::RollbackFailed(
                "previous model id missing".to_string(),
            ));
        }
        Ok(self.previous_model)
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
    ArtifactInvalid(ArtifactError),
    /// Resource budget was exceeded.
    #[error("budget exceeded: used {used:?} > limit {limit:?}")]
    BudgetExceeded {
        /// Actual usage.
        used: ResourceBudget,
        /// Enforced limit.
        limit: ResourceBudget,
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
    /// Online adaptation is not implemented in this engine.
    #[error("online adaptation not implemented: {0}")]
    AdaptationNotImplemented(String),
    /// Internal inference failure not covered by other variants.
    #[error("internal error: {0}")]
    InternalError(String),
}

impl From<InferenceError> for themql_core::Error {
    fn from(e: InferenceError) -> Self {
        themql_core::Error::internal_error(e.to_string())
    }
}

impl From<ArtifactError> for InferenceError {
    fn from(e: ArtifactError) -> Self {
        Self::ArtifactInvalid(e)
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
// TchInferenceEngine — tch-backed inference (behind `tch-backend` feature)
// ===========================================================================

/// tch-backed [`InferenceEngine`]. Activates a [`ModelArtifact`] after
/// running the full `themql-artifact::HashValidator` validation pipeline,
/// deserialises the model bytes into a `tch::CModule` (pre-allocated at
/// activation), and runs a real forward pass per `infer()`. Per
/// `specs/inference.toml`, tch is the primary tensor framework on
/// embedded; inference is augmentative only and never the authoritative
/// flight-control path.
#[cfg(feature = "tch-backend")]
pub struct TchInferenceEngine {
    active: Option<ModelArtifact>,
    module: Option<tch::CModule>,
    previous: Option<ModelArtifact>,
}

#[cfg(feature = "tch-backend")]
impl TchInferenceEngine {
    /// Construct a new engine with no model loaded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: None,
            module: None,
            previous: None,
        }
    }
}

#[cfg(feature = "tch-backend")]
impl Default for TchInferenceEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "tch-backend")]
#[allow(clippy::unwrap_used, clippy::expect_used)]
impl InferenceEngine for TchInferenceEngine {
    fn load(&mut self, artifact: ModelArtifact) -> Result<ActivationHandle, InferenceError> {
        if artifact.model_bytes.is_empty() {
            return Err(InferenceError::ArtifactInvalid(
                ArtifactError::ValidationFailed("empty model bytes".to_string()),
            ));
        }
        let validator = themql_artifact::HashValidator::new();
        let report = validator
            .validate(&artifact)
            .map_err(InferenceError::from)?;
        if !report.errors.is_empty() {
            let msg = report
                .errors
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(InferenceError::ArtifactInvalid(
                ArtifactError::ValidationFailed(msg),
            ));
        }
        let mut cursor = std::io::Cursor::new(&artifact.model_bytes);
        let cmodule = tch::CModule::load_data(&mut cursor).map_err(|e| {
            InferenceError::ArtifactInvalid(ArtifactError::ValidationFailed(e.to_string()))
        })?;
        let model_id = artifact.metadata.model_id.clone();
        if let Some(prev) = self.active.take() {
            self.previous = Some(prev);
        }
        self.module = Some(cmodule);
        self.active = Some(artifact);
        Ok(ActivationHandle {
            model_id,
            activated_at: Timestamp::now_monotonic(),
        })
    }

    fn infer(&self, input: &InferenceInput) -> Result<InferenceOutput, InferenceError> {
        let active = self.active.as_ref().ok_or(InferenceError::ModelNotLoaded)?;
        let module = self.module.as_ref().ok_or(InferenceError::ModelNotLoaded)?;
        if active.model_bytes.is_empty() {
            return Err(InferenceError::ArtifactInvalid(
                ArtifactError::ValidationFailed("empty model bytes on active artifact".to_string()),
            ));
        }
        let state_slice: &[f64] = input.state.as_slice();
        let len = i64::try_from(state_slice.len())
            .map_err(|_| InferenceError::InferenceFailed("state length overflow".to_string()))?;
        let start = std::time::Instant::now();
        let input_t = tch::Tensor::from_slice(state_slice)
            .to_kind(tch::Kind::Float)
            .reshape([1, len]);
        let out_t = tch::no_grad(|| module.forward_ts(&[input_t]))
            .map_err(|e| InferenceError::InferenceFailed(e.to_string()))?;
        let out_len = out_t.numel();
        let mut buf = vec![0_f32; out_len.min(STATE_DIM)];
        let copy_n = buf.len();
        let _ = out_t
            .to_kind(tch::Kind::Float)
            .f_copy_data(&mut buf, copy_n)
            .map_err(|e| InferenceError::InferenceFailed(e.to_string()))?;
        let mut correction = nalgebra::SVector::<f64, STATE_DIM>::zeros();
        for (i, v) in buf.iter().enumerate() {
            if i < STATE_DIM {
                correction[i] = f64::from(*v);
            }
        }
        let latency_ns = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        let latency_ms = u16::try_from(start.elapsed().as_millis()).unwrap_or(u16::MAX);
        if latency_ms > input.budget.inference_deadline_ms {
            return Err(InferenceError::DeadlineExceeded {
                deadline_ms: input.budget.inference_deadline_ms,
                actual_ms: latency_ms,
            });
        }
        let norm: f64 = correction
            .iter()
            .map(|v| {
                let av = v.abs();
                av * av
            })
            .sum::<f64>()
            .sqrt();
        let confidence = 1.0 / (1.0 + norm);
        Ok(InferenceOutput {
            state_correction: correction,
            confidence,
            latency_ns,
        })
    }

    fn rollback(&mut self, handle: RollbackHandle) -> Result<(), InferenceError> {
        let model = handle.restore()?;
        let prev = self.previous.take().ok_or_else(|| {
            InferenceError::RollbackFailed("no previous model retained".to_string())
        })?;
        let mut cursor = std::io::Cursor::new(&model.model_bytes);
        let cmodule = tch::CModule::load_data(&mut cursor)
            .map_err(|e| InferenceError::RollbackFailed(e.to_string()))?;
        self.module = Some(cmodule);
        self.active = Some(prev);
        Ok(())
    }

    fn active_model(&self) -> Option<&ModelArtifact> {
        self.active.as_ref()
    }
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
    fn rollback_handle_restore_returns_model() {
        let model = themql_schema::TrainedModel {
            model_bytes: vec![1, 2, 3],
            format: themql_schema::ModelFormat::TorchScript,
            feature_schema: themql_schema::FeatureSchema {
                features: vec![themql_schema::FeatureSpec {
                    name: "state".to_string(),
                    dtype: themql_schema::FeatureDType::F32,
                    shape: vec![21],
                }],
                normalization: themql_schema::NormalizationSpec::None,
            },
            validation_metrics: themql_schema::ValidationMetrics {
                loss: 0.0,
                accuracy: None,
                custom: std::collections::BTreeMap::new(),
            },
        };
        let h = RollbackHandle::new("m-1".to_string(), model.clone());
        let restored = h.restore().expect("restore");
        assert_eq!(restored, model);
    }

    #[test]
    fn rollback_handle_restore_rejects_empty_bytes() {
        let model = themql_schema::TrainedModel {
            model_bytes: Vec::new(),
            format: themql_schema::ModelFormat::TorchScript,
            feature_schema: themql_schema::FeatureSchema {
                features: vec![],
                normalization: themql_schema::NormalizationSpec::None,
            },
            validation_metrics: themql_schema::ValidationMetrics {
                loss: 0.0,
                accuracy: None,
                custom: std::collections::BTreeMap::new(),
            },
        };
        let h = RollbackHandle::new("m-1".to_string(), model);
        assert!(h.restore().is_err());
    }

    #[test]
    fn rollback_handle_restore_rejects_missing_id() {
        let model = themql_schema::TrainedModel {
            model_bytes: vec![1, 2, 3],
            format: themql_schema::ModelFormat::TorchScript,
            feature_schema: themql_schema::FeatureSchema {
                features: vec![],
                normalization: themql_schema::NormalizationSpec::None,
            },
            validation_metrics: themql_schema::ValidationMetrics {
                loss: 0.0,
                accuracy: None,
                custom: std::collections::BTreeMap::new(),
            },
        };
        let h = RollbackHandle::new(String::new(), model);
        assert!(h.restore().is_err());
    }

    #[cfg(feature = "tch-backend")]
    mod tch_backend {
        #![allow(clippy::unwrap_used, clippy::expect_used)]
        use super::*;
        use std::collections::BTreeMap;
        use themql_artifact::{ArtifactMetadata, CompatibilityInfo, TensorSchema};
        use themql_schema::{
            FeatureDType, FeatureSchema, FeatureSpec, ModelFormat, NormalizationSpec,
            ValidationMetrics,
        };

        fn sample_artifact() -> ModelArtifact {
            let metadata = ArtifactMetadata {
                model_id: "pinn-v1".to_string(),
                training_version: "0.1".to_string(),
                dataset_version: "ds-0".to_string(),
                feature_schema: FeatureSchema {
                    features: vec![FeatureSpec {
                        name: "state".to_string(),
                        dtype: FeatureDType::F32,
                        shape: vec![21],
                    }],
                    normalization: NormalizationSpec::None,
                },
                normalization: NormalizationSpec::None,
                validation_metrics: ValidationMetrics {
                    loss: 0.1,
                    accuracy: Some(0.9),
                    custom: BTreeMap::new(),
                },
                pruning_metadata: None,
            };
            ModelArtifact {
                model_bytes: vec![1, 2, 3, 4],
                format: ModelFormat::SafeTensors,
                hash: [0u8; 32],
                metadata,
                compatibility: CompatibilityInfo {
                    runtime_version: "0.1".to_string(),
                    architecture_version: "pinn-v1".to_string(),
                    tensor_schema: TensorSchema {
                        input_shapes: vec![vec![21]],
                        output_shapes: vec![vec![21]],
                        dtype: "f32".to_string(),
                    },
                },
                schema_version: 1,
            }
        }

        #[test]
        fn tch_inference_engine_infer_returns_valid_output() {
            let mut engine = TchInferenceEngine::new();
            let artifact = sample_artifact();
            engine.load(artifact).expect("load");
            let input = InferenceInput {
                state: nalgebra::SVector::<f64, STATE_DIM>::zeros(),
                budget: ResourceBudget::new(50, 1024, 10, 100).expect("budget"),
            };
            let out = engine.infer(&input).expect("infer");
            assert_eq!(out.confidence, 1.0);
            assert_eq!(out.state_correction.len(), STATE_DIM);
        }

        #[test]
        fn tch_inference_engine_rejects_infer_without_model() {
            let engine = TchInferenceEngine::new();
            let input = InferenceInput {
                state: nalgebra::SVector::<f64, STATE_DIM>::zeros(),
                budget: ResourceBudget::new(50, 1024, 10, 100).expect("budget"),
            };
            assert!(matches!(
                engine.infer(&input),
                Err(InferenceError::ModelNotLoaded)
            ));
        }
    }
}
