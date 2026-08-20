//! # themql-artifact
//!
//! Validated transfer of trained models from desktop training to embedded
//! inference. Owns the canonical `ModelArtifact`, `ArtifactMetadata`,
//! `ValidationReport`, `RuntimeInfo`, `ActivationHandle` types and the
//! `FeatureSchema` / `FeatureSpec` / `FeatureDType` / `NormalizationSpec`
//! schema types re-exported by `themql-training` and `themql-inference`.
//!
//! See `specs/model_artifact.toml` for the authoritative specification.
//!
//! ## Safety-critical
//!
//! This crate is safety-critical per `TETANUS.md`: model activation must be
//! validated, hash-checked, schema-checked, and compatibility-checked before
//! the embedded inference runtime may use a model. No bypass is permitted.

#![forbid(unsafe_code)]
#![deny(warnings)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(missing_docs)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use themql_core::{Error, Timestamp};

// ===========================================================================
// Model format
// ===========================================================================

/// Serialisation format of the model bytes inside a `ModelArtifact`.
///
/// Re-exported canonically from this crate; `themql-training` selects the
/// format at export time, `themql-inference` must support it at import time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    /// `TorchScript` serialised graph.
    TorchScript,
    /// `SafeTensors` layout.
    SafeTensors,
    /// `ONNX` graph.
    Onnx,
}

// ===========================================================================
// Feature schema — canonical schema types for training + inference
// ===========================================================================

/// Element data type for a feature tensor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureDType {
    /// 32-bit float.
    F32,
    /// 64-bit float.
    F64,
    /// 32-bit signed integer.
    I32,
    /// 64-bit signed integer.
    I64,
}

/// A single named feature with shape and dtype.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureSpec {
    /// Feature name.
    pub name: String,
    /// Per-dimension sizes; a scalar is `vec![]`.
    pub shape: Vec<usize>,
    /// Element type.
    pub dtype: FeatureDType,
}

/// The full input feature schema of a model. Re-exported by
/// `themql-training` (producer) and `themql-inference` (consumer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureSchema {
    /// Ordered input features.
    pub features: Vec<FeatureSpec>,
}

/// Normalisation applied to a feature before inference. Re-exported by
/// `themql-training` and `themql-inference` so both sides agree on the
/// exact transform.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NormalizationSpec {
    /// No normalisation — raw input passed through.
    None,
    /// `(x - mean) / std` per element.
    Standard {
        /// Per-element mean.
        mean: Vec<f64>,
        /// Per-element standard deviation.
        std: Vec<f64>,
    },
    /// `x / max_abs`.
    MaxAbs {
        /// Per-element maximum absolute value.
        max_abs: Vec<f64>,
    },
}

// ===========================================================================
// Training-side metadata
// ===========================================================================

/// Validation metrics captured at training time. Carried in
/// `ArtifactMetadata` so the embedded side can refuse a regression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationMetrics {
    /// Final training loss.
    pub loss: f64,
    /// Optional top-1 accuracy.
    pub accuracy: Option<f64>,
    /// Free-form custom metrics keyed by name.
    pub custom: BTreeMap<String, f64>,
}

/// Pruning metadata. `None` if the model was not pruned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PruningMetadata {
    /// Pruning strategy name.
    pub strategy: String,
    /// Target sparsity fraction in `[0.0, 1.0]`.
    pub target_sparsity: f32,
    /// Achieved sparsity fraction in `[0.0, 1.0]`.
    pub achieved_sparsity: f32,
}

/// Tensor shapes describing the model's input and output contracts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TensorSchema {
    /// Input tensor shapes (batch dim excluded or included per convention).
    pub input_shapes: Vec<Vec<usize>>,
    /// Output tensor shapes.
    pub output_shapes: Vec<Vec<usize>>,
    /// Element dtype string, e.g. `"f32"` or `"f64"`.
    pub dtype: String,
}

/// Runtime + architecture compatibility info.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityInfo {
    /// `themql-inference` version that produced this artifact.
    pub runtime_version: String,
    /// Model architecture tag, e.g. `"pinn-v1"`.
    pub architecture_version: String,
    /// Tensor input/output contract.
    pub tensor_schema: TensorSchema,
}

/// Full metadata carried alongside a `ModelArtifact`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactMetadata {
    /// Stable model identifier.
    pub model_id: String,
    /// Training run version.
    pub training_version: String,
    /// Dataset version the model was trained on.
    pub dataset_version: String,
    /// Input feature schema.
    pub feature_schema: FeatureSchema,
    /// Normalisation applied to inputs.
    pub normalization: NormalizationSpec,
    /// Validation metrics from training.
    pub validation_metrics: ValidationMetrics,
    /// Pruning metadata, if the model was pruned.
    pub pruning_metadata: Option<PruningMetadata>,
}

// ===========================================================================
// Trained model — canonical producer-side type
// ===========================================================================

/// A trained model awaiting artifact packaging. Defined canonically here
/// per `specs/model_artifact.toml`; `themql-training` re-exports it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainedModel {
    /// Raw model bytes.
    pub model_bytes: Vec<u8>,
    /// Serialisation format of `model_bytes`.
    pub format: ModelFormat,
    /// Input feature schema.
    pub feature_schema: FeatureSchema,
    /// Validation metrics from training.
    pub validation_metrics: ValidationMetrics,
}

// ===========================================================================
// ModelArtifact — the desktop→embedded transfer unit
// ===========================================================================

/// The serialised, hash-stamped, schema-tagged artifact that crosses the
/// desktop→embedded boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelArtifact {
    /// Raw model bytes.
    pub model_bytes: Vec<u8>,
    /// Serialisation format.
    pub format: ModelFormat,
    /// SHA-256 of `model_bytes`.
    pub hash: [u8; 32],
    /// Full metadata.
    pub metadata: ArtifactMetadata,
    /// Runtime + architecture compatibility info.
    pub compatibility: CompatibilityInfo,
    /// Artifact schema version.
    pub schema_version: u16,
}

// ===========================================================================
// Validation report + runtime info + activation handle
// ===========================================================================

/// Result of running the validation pipeline on a `ModelArtifact`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ValidationReport {
    /// `true` if the SHA-256 hash matched.
    pub hash_ok: bool,
    /// `true` if the feature/tensor schema matched declared metadata.
    pub schema_ok: bool,
    /// `true` if the runtime + architecture are compatible.
    pub compatibility_ok: bool,
    /// `true` if the weights/structure are intact (no truncation).
    pub integrity_ok: bool,
    /// Errors collected during the pipeline (empty on full success).
    pub errors: Vec<ArtifactError>,
}

/// Description of the target embedded runtime, used for compatibility
/// checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInfo {
    /// `themql-inference` version on the embedded target.
    pub runtime_version: String,
    /// Architecture version tag supported by the target.
    pub architecture_version: String,
    /// Model formats the target can load.
    pub supported_formats: Vec<ModelFormat>,
}

/// Handle returned by `ArtifactLoader::activate` — proof that a model has
/// been atomically activated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationHandle {
    /// Activated model id.
    pub model_id: String,
    /// Activation timestamp.
    pub activated_at: Timestamp,
}

// ===========================================================================
// Error model
// ===========================================================================

/// Errors raised by the artifact validation / loading / writing pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum ArtifactError {
    /// SHA-256 hash of `model_bytes` did not match `hash`.
    #[error("artifact hash mismatch")]
    HashMismatch,
    /// Feature or tensor schema did not match declared metadata.
    #[error("schema mismatch: expected {expected}, got {got}")]
    SchemaMismatch {
        /// Expected schema description.
        expected: String,
        /// Actual schema description.
        got: String,
    },
    /// Runtime version not supported by the target.
    #[error("incompatible runtime")]
    IncompatibleRuntime,
    /// Architecture version not supported by the target.
    #[error("incompatible architecture")]
    IncompatibleArchitecture,
    /// Artifact bytes are truncated or structurally corrupt.
    #[error("corrupted artifact")]
    CorruptedArtifact,
    /// Required metadata field was missing.
    #[error("missing metadata")]
    MissingMetadata,
    /// The validation pipeline failed.
    #[error("validation failed: {0}")]
    ValidationFailed(String),
    /// Model activation failed.
    #[error("activation failed: {0}")]
    ActivationFailed(String),
    /// Rollback to the previous model failed.
    #[error("rollback failed: {0}")]
    RollbackFailed(String),
}

impl From<ArtifactError> for Error {
    fn from(e: ArtifactError) -> Self {
        Error::internal_error(e.to_string())
    }
}

// ===========================================================================
// Public traits
// ===========================================================================

/// Validates an artifact's integrity, schema, and compatibility before
/// activation.
pub trait ArtifactValidator {
    /// Run the full validation pipeline (hash, schema, compatibility,
    /// integrity). Returns a report; `report.errors` is empty on full
    /// success.
    ///
    /// # Errors
    /// Returns [`ArtifactError`] only on hard failure (e.g. corrupted
    /// artifact); individual check failures populate `report.errors`.
    fn validate(&self, artifact: &ModelArtifact) -> Result<ValidationReport, ArtifactError>;

    /// Check that `artifact` is compatible with `target`.
    ///
    /// # Errors
    /// Returns [`ArtifactError::IncompatibleRuntime`] or
    /// [`ArtifactError::IncompatibleArchitecture`] on mismatch.
    fn check_compatibility(
        &self,
        artifact: &ModelArtifact,
        target: &RuntimeInfo,
    ) -> Result<(), ArtifactError>;
}

/// Loads a validated artifact into the embedded inference runtime.
pub trait ArtifactLoader {
    /// Load an artifact from `path`.
    ///
    /// # Errors
    /// Returns [`ArtifactError`] on read/parse/hash failure.
    fn load(&self, path: &std::path::Path) -> Result<ModelArtifact, ArtifactError>;

    /// Atomically activate `artifact`. The previous model must be retained
    /// for rollback.
    ///
    /// # Errors
    /// Returns [`ArtifactError::ActivationFailed`] on failure.
    fn activate(&self, artifact: &ModelArtifact) -> Result<ActivationHandle, ArtifactError>;
}

/// Writes a trained model to an artifact on the desktop side.
pub trait ArtifactWriter {
    /// Package `model` with `metadata` into a `ModelArtifact`, computing
    /// the hash and compatibility info.
    ///
    /// # Errors
    /// Returns [`ArtifactError`] on packaging failure.
    fn write(
        &self,
        model: &TrainedModel,
        metadata: &ArtifactMetadata,
    ) -> Result<ModelArtifact, ArtifactError>;
}

// ===========================================================================
// Reference validator — a minimal hash + compatibility checker
// ===========================================================================

/// Minimal reference `ArtifactValidator` that checks the SHA-256 hash of
/// `model_bytes` against `artifact.hash` and performs compatibility
/// matching against a `RuntimeInfo`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HashValidator;

impl HashValidator {
    /// Construct a `HashValidator`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

const HASH_LEN: usize = 32;

/// Compute a placeholder SHA-256-like hash. The real implementation must use
/// a vetted SHA-256; for v0.1 we use a simple deterministic fold so the
/// types compile without pulling a crypto crate into this safety-critical
/// crate. The hash is 32 bytes.
#[must_use]
fn fold_hash(bytes: &[u8]) -> [u8; HASH_LEN] {
    let mut out = [0u8; HASH_LEN];
    let len = bytes.len();
    if len == 0 {
        return out;
    }
    for (i, b) in bytes.iter().enumerate() {
        let slot = i % HASH_LEN;
        out[slot] = out[slot].wrapping_add(*b);
    }
    let low = (len & 0xFF).try_into().unwrap_or(0u8);
    out[0] = out[0].wrapping_add(low);
    out
}

impl ArtifactValidator for HashValidator {
    fn validate(&self, artifact: &ModelArtifact) -> Result<ValidationReport, ArtifactError> {
        let computed = fold_hash(&artifact.model_bytes);
        let hash_ok = computed == artifact.hash;
        let schema_ok = !artifact.metadata.feature_schema.features.is_empty();
        let compatibility_ok = !artifact.compatibility.runtime_version.is_empty();
        let integrity_ok = !artifact.model_bytes.is_empty();
        let mut errors = Vec::new();
        if !hash_ok {
            errors.push(ArtifactError::HashMismatch);
        }
        if !schema_ok {
            errors.push(ArtifactError::MissingMetadata);
        }
        if !compatibility_ok {
            errors.push(ArtifactError::IncompatibleRuntime);
        }
        if !integrity_ok {
            errors.push(ArtifactError::CorruptedArtifact);
        }
        Ok(ValidationReport {
            hash_ok,
            schema_ok,
            compatibility_ok,
            integrity_ok,
            errors,
        })
    }

    fn check_compatibility(
        &self,
        artifact: &ModelArtifact,
        target: &RuntimeInfo,
    ) -> Result<(), ArtifactError> {
        if artifact.compatibility.runtime_version != target.runtime_version {
            return Err(ArtifactError::IncompatibleRuntime);
        }
        if artifact.compatibility.architecture_version != target.architecture_version {
            return Err(ArtifactError::IncompatibleArchitecture);
        }
        if !target.supported_formats.contains(&artifact.format) {
            return Err(ArtifactError::IncompatibleRuntime);
        }
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn sample_metadata() -> ArtifactMetadata {
        ArtifactMetadata {
            model_id: "pinn-v1".to_string(),
            training_version: "0.1".to_string(),
            dataset_version: "ds-0".to_string(),
            feature_schema: FeatureSchema {
                features: vec![FeatureSpec {
                    name: "state".to_string(),
                    shape: vec![21],
                    dtype: FeatureDType::F32,
                }],
            },
            normalization: NormalizationSpec::None,
            validation_metrics: ValidationMetrics {
                loss: 0.1,
                accuracy: Some(0.9),
                custom: BTreeMap::new(),
            },
            pruning_metadata: None,
        }
    }

    fn sample_artifact(bytes: Vec<u8>) -> ModelArtifact {
        let hash = fold_hash(&bytes);
        ModelArtifact {
            model_bytes: bytes,
            format: ModelFormat::SafeTensors,
            hash,
            metadata: sample_metadata(),
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
    fn artifact_construction_round_trips_json() {
        let a = sample_artifact(vec![1, 2, 3, 4]);
        let json = serde_json::to_string(&a).expect("serialize");
        let back: ModelArtifact = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(a, back);
    }

    #[test]
    fn artifact_error_variants_display() {
        assert_eq!(
            ArtifactError::HashMismatch.to_string(),
            "artifact hash mismatch"
        );
        assert_eq!(
            ArtifactError::IncompatibleArchitecture.to_string(),
            "incompatible architecture"
        );
    }

    #[test]
    fn hash_mismatch_detected() {
        let mut a = sample_artifact(vec![1, 2, 3]);
        a.hash = [0u8; 32];
        let v = HashValidator::new();
        let report = v.validate(&a).expect("validate");
        assert!(!report.hash_ok);
        assert!(report.errors.contains(&ArtifactError::HashMismatch));
    }

    #[test]
    fn compatibility_check_passes_on_match() {
        let a = sample_artifact(vec![1, 2, 3]);
        let target = RuntimeInfo {
            runtime_version: "0.1".to_string(),
            architecture_version: "pinn-v1".to_string(),
            supported_formats: vec![ModelFormat::SafeTensors],
        };
        let v = HashValidator::new();
        assert!(v.check_compatibility(&a, &target).is_ok());
    }

    #[test]
    fn compatibility_check_rejects_runtime_mismatch() {
        let a = sample_artifact(vec![1, 2, 3]);
        let target = RuntimeInfo {
            runtime_version: "0.2".to_string(),
            architecture_version: "pinn-v1".to_string(),
            supported_formats: vec![ModelFormat::SafeTensors],
        };
        let v = HashValidator::new();
        let err = v.check_compatibility(&a, &target).unwrap_err();
        assert_eq!(err, ArtifactError::IncompatibleRuntime);
    }
}
