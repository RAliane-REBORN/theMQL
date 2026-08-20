//! # themql-artifact
//!
//! Validated transfer of trained models from desktop training to embedded
//! inference. Owns the canonical `ModelArtifact`, `ArtifactMetadata`,
//! `ValidationReport`, `RuntimeInfo`, `ActivationHandle` types. The
//! `FeatureSchema` / `FeatureSpec` / `FeatureDType` / `NormalizationSpec`
//! / `ModelFormat` / `ValidationMetrics` / `TrainedModel` schema types
//! are owned by `themql-schema` and re-exported here.
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

use std::path::Path;

use serde::{Deserialize, Serialize};
use themql_core::{Error, Timestamp};

// Re-export the shared schema types so downstream crates can depend on
// themql-artifact alone for all artifact-related types.
pub use themql_schema::{
    FeatureDType, FeatureSchema, FeatureSpec, ModelFormat, NormalizationSpec, TrainedModel,
    ValidationMetrics,
};

// ===========================================================================
// Pruning metadata
// ===========================================================================

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

// ===========================================================================
// Tensor + compatibility info
// ===========================================================================

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

// ===========================================================================
// ArtifactMetadata
// ===========================================================================

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
    /// blake3 hash of `model_bytes` (32 bytes).
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
    /// `true` if the blake3 hash matched.
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
    /// blake3 hash of `model_bytes` did not match `hash`.
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
    /// I/O error during load or write.
    #[error("io error: {0}")]
    Io(String),
    /// Serialisation or deserialisation error.
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl From<ArtifactError> for Error {
    fn from(e: ArtifactError) -> Self {
        Error::internal_error(e.to_string())
    }
}

impl From<std::io::Error> for ArtifactError {
    fn from(e: std::io::Error) -> Self {
        ArtifactError::Io(e.to_string())
    }
}

impl From<bincode::Error> for ArtifactError {
    fn from(e: bincode::Error) -> Self {
        ArtifactError::Serialization(e.to_string())
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
    fn load(&self, path: &Path) -> Result<ModelArtifact, ArtifactError>;

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
// blake3 hash helper
// ===========================================================================

const HASH_LEN: usize = 32;

/// Compute the blake3 hash of `bytes`, returning a 32-byte digest.
#[must_use]
fn blake3_hash(bytes: &[u8]) -> [u8; HASH_LEN] {
    blake3::hash(bytes).into()
}

// ===========================================================================
// HashValidator — real blake3-based validator
// ===========================================================================

/// `ArtifactValidator` that checks the blake3 hash of `model_bytes`
/// against `artifact.hash`, validates the feature schema against the
/// tensor schema, checks compatibility, and verifies structural
/// integrity. The pipeline short-circuits on the first failure per
/// `specs/model_artifact.toml [validation_pipeline]`.
#[derive(Debug, Clone)]
pub struct HashValidator {
    /// Target runtime info for compatibility checks.
    target: Option<RuntimeInfo>,
}

impl HashValidator {
    /// Construct a `HashValidator` without a target runtime.
    #[must_use]
    pub fn new() -> Self {
        Self { target: None }
    }

    /// Construct a `HashValidator` with a target `RuntimeInfo` so
    /// `validate` also runs the compatibility check.
    #[must_use]
    pub fn with_target(target: RuntimeInfo) -> Self {
        Self {
            target: Some(target),
        }
    }
}

impl Default for HashValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Verify that the feature schema's total element count matches the
/// tensor schema's input shape product.
fn validate_schema(artifact: &ModelArtifact) -> Result<bool, ArtifactError> {
    let features = &artifact.metadata.feature_schema.features;
    if features.is_empty() {
        return Ok(false);
    }
    let tensor_input = &artifact.compatibility.tensor_schema.input_shapes;
    if tensor_input.is_empty() {
        return Ok(false);
    }
    let feature_elements: usize = features
        .iter()
        .map(|f| f.shape.iter().product::<usize>().max(1))
        .sum();
    let tensor_elements: usize = tensor_input
        .iter()
        .map(|s| s.iter().product::<usize>().max(1))
        .sum();
    if feature_elements != tensor_elements {
        return Err(ArtifactError::SchemaMismatch {
            expected: format!("{tensor_elements} tensor input elements"),
            got: format!("{feature_elements} feature elements"),
        });
    }
    Ok(true)
}

/// Verify that the model bytes are non-empty and not obviously
/// truncated (minimal structural check: at least 4 bytes).
fn check_integrity(artifact: &ModelArtifact) -> bool {
    artifact.model_bytes.len() >= 4
}

impl ArtifactValidator for HashValidator {
    fn validate(&self, artifact: &ModelArtifact) -> Result<ValidationReport, ArtifactError> {
        let mut report = ValidationReport {
            hash_ok: false,
            schema_ok: false,
            compatibility_ok: false,
            integrity_ok: false,
            errors: Vec::new(),
        };

        // Step 1: hash_verify (short-circuit on failure)
        let computed = blake3_hash(&artifact.model_bytes);
        if computed != artifact.hash {
            report.hash_ok = false;
            report.errors.push(ArtifactError::HashMismatch);
            return Ok(report);
        }
        report.hash_ok = true;

        // Step 2: schema_validate (short-circuit on failure)
        match validate_schema(artifact) {
            Ok(true) => {
                report.schema_ok = true;
            }
            Ok(false) => {
                report.schema_ok = false;
                report.errors.push(ArtifactError::MissingMetadata);
                return Ok(report);
            }
            Err(e) => {
                report.schema_ok = false;
                report.errors.push(e);
                return Ok(report);
            }
        }

        // Step 3: compatibility_check (short-circuit on failure)
        if let Some(ref target) = self.target {
            match Self::check_compat_internal(artifact, target) {
                Ok(()) => {
                    report.compatibility_ok = true;
                }
                Err(e) => {
                    report.compatibility_ok = false;
                    report.errors.push(e);
                    return Ok(report);
                }
            }
        } else {
            report.compatibility_ok = !artifact.compatibility.runtime_version.is_empty();
            if !report.compatibility_ok {
                report.errors.push(ArtifactError::IncompatibleRuntime);
                return Ok(report);
            }
        }

        // Step 4: integrity_check
        report.integrity_ok = check_integrity(artifact);
        if !report.integrity_ok {
            report.errors.push(ArtifactError::CorruptedArtifact);
        }

        Ok(report)
    }

    fn check_compatibility(
        &self,
        artifact: &ModelArtifact,
        target: &RuntimeInfo,
    ) -> Result<(), ArtifactError> {
        Self::check_compat_internal(artifact, target)
    }
}

impl HashValidator {
    fn check_compat_internal(
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
// FileArtifactLoader — loads artifacts from bincode files
// ===========================================================================

/// `ArtifactLoader` that reads a bincode-serialised `ModelArtifact` from
/// a file path, recomputes the hash, and validates it before returning.
#[derive(Debug, Clone, Default)]
pub struct FileArtifactLoader {
    /// Validator used to check the artifact after loading.
    validator: HashValidator,
}

impl FileArtifactLoader {
    /// Construct a `FileArtifactLoader` with a default `HashValidator`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a `FileArtifactLoader` with a target `RuntimeInfo` for
    /// compatibility checking during load.
    #[must_use]
    pub fn with_target(target: RuntimeInfo) -> Self {
        Self {
            validator: HashValidator::with_target(target),
        }
    }
}

impl ArtifactLoader for FileArtifactLoader {
    fn load(&self, path: &Path) -> Result<ModelArtifact, ArtifactError> {
        let bytes = std::fs::read(path)?;
        let artifact: ModelArtifact = bincode::deserialize(&bytes)?;
        let report = self.validator.validate(&artifact)?;
        if !report.errors.is_empty() {
            return Err(ArtifactError::ValidationFailed(
                report
                    .errors
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }
        Ok(artifact)
    }

    fn activate(&self, artifact: &ModelArtifact) -> Result<ActivationHandle, ArtifactError> {
        if artifact.model_bytes.is_empty() {
            return Err(ArtifactError::ActivationFailed(
                "empty model bytes".to_string(),
            ));
        }
        let report = self.validator.validate(artifact)?;
        if !report.errors.is_empty() {
            return Err(ArtifactError::ActivationFailed(
                report
                    .errors
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }
        Ok(ActivationHandle {
            model_id: artifact.metadata.model_id.clone(),
            activated_at: Timestamp::now_wall_clock(),
        })
    }
}

// ===========================================================================
// BincodeArtifactWriter — packages TrainedModel into ModelArtifact
// ===========================================================================

/// `ArtifactWriter` that packages a `TrainedModel` and `ArtifactMetadata`
/// into a validated `ModelArtifact`, computing the blake3 hash and
/// building the `CompatibilityInfo`.
#[derive(Debug, Clone, Default)]
pub struct BincodeArtifactWriter {
    /// Runtime version of the producing training environment.
    runtime_version: String,
    /// Architecture version tag.
    architecture_version: String,
}

impl BincodeArtifactWriter {
    /// Construct a `BincodeArtifactWriter` with the given runtime and
    /// architecture versions.
    #[must_use]
    pub fn new(runtime_version: &str, architecture_version: &str) -> Self {
        Self {
            runtime_version: runtime_version.to_string(),
            architecture_version: architecture_version.to_string(),
        }
    }

    /// Write the `ModelArtifact` to a file as bincode.
    ///
    /// # Errors
    /// Returns [`ArtifactError`] on serialisation or I/O failure.
    pub fn write_to_file(
        &self,
        artifact: &ModelArtifact,
        path: &Path,
    ) -> Result<(), ArtifactError> {
        let bytes = bincode::serialize(artifact)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

impl ArtifactWriter for BincodeArtifactWriter {
    fn write(
        &self,
        model: &TrainedModel,
        metadata: &ArtifactMetadata,
    ) -> Result<ModelArtifact, ArtifactError> {
        if model.model_bytes.is_empty() {
            return Err(ArtifactError::CorruptedArtifact);
        }
        let hash = blake3_hash(&model.model_bytes);
        let total_input_elements: usize = metadata
            .feature_schema
            .features
            .iter()
            .map(|f| f.shape.iter().product::<usize>().max(1))
            .sum();
        let tensor_schema = TensorSchema {
            input_shapes: vec![vec![total_input_elements]],
            output_shapes: vec![vec![total_input_elements]],
            dtype: "f32".to_string(),
        };
        Ok(ModelArtifact {
            model_bytes: model.model_bytes.clone(),
            format: model.format,
            hash,
            metadata: metadata.clone(),
            compatibility: CompatibilityInfo {
                runtime_version: self.runtime_version.clone(),
                architecture_version: self.architecture_version.clone(),
                tensor_schema,
            },
            schema_version: 1,
        })
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::similar_names)]
    use super::*;
    use std::collections::BTreeMap;

    fn sample_metadata() -> ArtifactMetadata {
        ArtifactMetadata {
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
        }
    }

    fn sample_artifact(bytes: Vec<u8>) -> ModelArtifact {
        let hash = blake3_hash(&bytes);
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
    fn artifact_bincode_round_trips() {
        let a = sample_artifact(vec![1, 2, 3, 4, 5, 6]);
        let bytes = bincode::serialize(&a).expect("serialize");
        let back: ModelArtifact = bincode::deserialize(&bytes).expect("deserialize");
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
    fn hash_match_passes() {
        let a = sample_artifact(vec![1, 2, 3, 4, 5, 6]);
        let v = HashValidator::new();
        let report = v.validate(&a).expect("validate");
        assert!(report.hash_ok);
        assert!(report.schema_ok);
        assert!(report.compatibility_ok);
        assert!(report.integrity_ok);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn short_circuit_on_hash_failure() {
        let mut a = sample_artifact(vec![1, 2, 3]);
        a.hash = [0u8; 32];
        let v = HashValidator::new();
        let report = v.validate(&a).expect("validate");
        assert!(!report.hash_ok);
        assert!(!report.schema_ok);
        assert!(!report.compatibility_ok);
        assert!(!report.integrity_ok);
        assert_eq!(report.errors.len(), 1);
    }

    #[test]
    fn schema_mismatch_detected() {
        let mut a = sample_artifact(vec![1, 2, 3, 4, 5, 6]);
        a.metadata.feature_schema.features = vec![FeatureSpec {
            name: "state".to_string(),
            dtype: FeatureDType::F32,
            shape: vec![99],
        }];
        let v = HashValidator::new();
        let report = v.validate(&a).expect("validate");
        assert!(report.hash_ok);
        assert!(!report.schema_ok);
        assert!(report
            .errors
            .iter()
            .any(|e| matches!(e, ArtifactError::SchemaMismatch { .. })));
    }

    #[test]
    fn empty_features_fails_schema() {
        let mut a = sample_artifact(vec![1, 2, 3, 4, 5, 6]);
        a.metadata.feature_schema.features = vec![];
        let v = HashValidator::new();
        let report = v.validate(&a).expect("validate");
        assert!(report.hash_ok);
        assert!(!report.schema_ok);
        assert!(report.errors.contains(&ArtifactError::MissingMetadata));
    }

    #[test]
    fn truncated_bytes_fail_integrity() {
        let a = sample_artifact(vec![1, 2]);
        let v = HashValidator::new();
        let report = v.validate(&a).expect("validate");
        assert!(report.hash_ok);
        assert!(report.schema_ok);
        assert!(report.compatibility_ok);
        assert!(!report.integrity_ok);
        assert!(report.errors.contains(&ArtifactError::CorruptedArtifact));
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

    #[test]
    fn compatibility_check_rejects_format_mismatch() {
        let a = sample_artifact(vec![1, 2, 3]);
        let target = RuntimeInfo {
            runtime_version: "0.1".to_string(),
            architecture_version: "pinn-v1".to_string(),
            supported_formats: vec![ModelFormat::Onnx],
        };
        let v = HashValidator::new();
        let err = v.check_compatibility(&a, &target).unwrap_err();
        assert_eq!(err, ArtifactError::IncompatibleRuntime);
    }

    #[test]
    fn writer_produces_valid_artifact() {
        let model = TrainedModel {
            model_bytes: vec![0xAB; 64],
            format: ModelFormat::SafeTensors,
            feature_schema: FeatureSchema {
                features: vec![FeatureSpec {
                    name: "state".to_string(),
                    dtype: FeatureDType::F32,
                    shape: vec![21],
                }],
                normalization: NormalizationSpec::None,
            },
            validation_metrics: ValidationMetrics {
                loss: 0.05,
                accuracy: Some(0.95),
                custom: BTreeMap::new(),
            },
        };
        let writer = BincodeArtifactWriter::new("0.1", "pinn-v1");
        let artifact = writer.write(&model, &sample_metadata()).expect("write");
        assert_eq!(artifact.model_bytes, model.model_bytes);
        assert_eq!(artifact.format, ModelFormat::SafeTensors);
        assert_eq!(artifact.hash, blake3_hash(&model.model_bytes));
        assert_eq!(artifact.compatibility.runtime_version, "0.1");
        assert_eq!(artifact.compatibility.architecture_version, "pinn-v1");
        assert_eq!(artifact.schema_version, 1);

        let v = HashValidator::new();
        let report = v.validate(&artifact).expect("validate");
        assert!(report.errors.is_empty());
    }

    #[test]
    fn writer_rejects_empty_model() {
        let model = TrainedModel {
            model_bytes: vec![],
            format: ModelFormat::SafeTensors,
            feature_schema: FeatureSchema {
                features: vec![FeatureSpec {
                    name: "state".to_string(),
                    dtype: FeatureDType::F32,
                    shape: vec![21],
                }],
                normalization: NormalizationSpec::None,
            },
            validation_metrics: ValidationMetrics {
                loss: 0.0,
                accuracy: None,
                custom: BTreeMap::new(),
            },
        };
        let writer = BincodeArtifactWriter::new("0.1", "pinn-v1");
        let err = writer.write(&model, &sample_metadata()).unwrap_err();
        assert_eq!(err, ArtifactError::CorruptedArtifact);
    }

    #[test]
    fn loader_loads_valid_artifact_from_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("themql_artifact_test_valid.bincode");
        let a = sample_artifact(vec![0xAB; 64]);
        let bytes = bincode::serialize(&a).expect("serialize");
        std::fs::write(&path, &bytes).expect("write");
        let loader = FileArtifactLoader::new();
        let loaded = loader.load(&path).expect("load");
        assert_eq!(loaded, a);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn loader_rejects_corrupt_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("themql_artifact_test_corrupt.bincode");
        std::fs::write(&path, b"not bincode").expect("write");
        let loader = FileArtifactLoader::new();
        assert!(loader.load(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn loader_activates_valid_artifact() {
        let a = sample_artifact(vec![0xAB; 64]);
        let loader = FileArtifactLoader::new();
        let handle = loader.activate(&a).expect("activate");
        assert_eq!(handle.model_id, "pinn-v1");
    }

    #[test]
    fn loader_rejects_activation_of_invalid_artifact() {
        let mut a = sample_artifact(vec![0xAB; 64]);
        a.hash = [0u8; 32];
        let loader = FileArtifactLoader::new();
        assert!(loader.activate(&a).is_err());
    }

    #[test]
    fn loader_rejects_activation_of_empty_bytes() {
        let mut a = sample_artifact(vec![1, 2, 3, 4]);
        a.model_bytes = vec![];
        let loader = FileArtifactLoader::new();
        assert!(loader.activate(&a).is_err());
    }

    #[test]
    fn writer_write_to_file_round_trips() {
        let dir = std::env::temp_dir();
        let path = dir.join("themql_artifact_test_writer.bincode");
        let model = TrainedModel {
            model_bytes: vec![0xCD; 128],
            format: ModelFormat::TorchScript,
            feature_schema: FeatureSchema {
                features: vec![FeatureSpec {
                    name: "state".to_string(),
                    dtype: FeatureDType::F32,
                    shape: vec![21],
                }],
                normalization: NormalizationSpec::None,
            },
            validation_metrics: ValidationMetrics {
                loss: 0.01,
                accuracy: Some(0.99),
                custom: BTreeMap::new(),
            },
        };
        let writer = BincodeArtifactWriter::new("0.1", "pinn-v1");
        let artifact = writer.write(&model, &sample_metadata()).expect("write");
        writer
            .write_to_file(&artifact, &path)
            .expect("write_to_file");
        let loader = FileArtifactLoader::new();
        let loaded = loader.load(&path).expect("load");
        assert_eq!(loaded, artifact);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn validate_with_target_runtime() {
        let a = sample_artifact(vec![0xAB; 64]);
        let target = RuntimeInfo {
            runtime_version: "0.1".to_string(),
            architecture_version: "pinn-v1".to_string(),
            supported_formats: vec![ModelFormat::SafeTensors],
        };
        let v = HashValidator::with_target(target);
        let report = v.validate(&a).expect("validate");
        assert!(report.errors.is_empty());
    }

    #[test]
    fn validate_with_target_runtime_rejects_mismatch() {
        let a = sample_artifact(vec![0xAB; 64]);
        let target = RuntimeInfo {
            runtime_version: "0.9".to_string(),
            architecture_version: "pinn-v1".to_string(),
            supported_formats: vec![ModelFormat::SafeTensors],
        };
        let v = HashValidator::with_target(target);
        let report = v.validate(&a).expect("validate");
        assert!(report.hash_ok);
        assert!(report.schema_ok);
        assert!(!report.compatibility_ok);
        assert!(report.errors.contains(&ArtifactError::IncompatibleRuntime));
    }
}
