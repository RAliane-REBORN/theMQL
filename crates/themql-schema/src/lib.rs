//! # themql-schema
//!
//! Canonical shared schema types for theMQL's training, artifact, and
//! inference crates. These types are defined here (not in
//! `themql-training` or `themql-artifact`) to avoid circular dependencies:
//! `themql-artifact` depends on this crate, and `themql-training` depends
//! on `themql-artifact`.
//!
//! Per `specs/training.toml`, the canonical owners of these types are the
//! training spec. This crate is the concrete Rust home for those spec
//! types; `themql-training`, `themql-artifact`, and `themql-inference`
//! all re-export from here.
//!
//! ## Types owned here
//!
//! - `ModelFormat` — serialisation format of model bytes
//! - `FeatureDType` — element type of a feature column
//! - `FeatureSpec` — a single named feature with shape and dtype
//! - `FeatureSchema` — ordered feature specs + normalisation
//! - `NormalizationSpec` — normalisation strategy
//! - `ValidationMetrics` — metrics from the validation pass
//! - `TrainedModel` — the producer-side model type

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ===========================================================================
// ModelFormat
// ===========================================================================

/// Serialisation format of the model bytes inside a `ModelArtifact`.
///
/// `themql-training` selects the format at export time; `themql-inference`
/// must support it at import time. Per `specs/training.toml
/// [api.TrainedModel]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    /// `TorchScript` serialised model.
    TorchScript,
    /// `SafeTensors` serialised model.
    SafeTensors,
    /// `ONNX` serialised model.
    Onnx,
}

// ===========================================================================
// FeatureDType
// ===========================================================================

/// Element type of a feature column. Per `specs/training.toml
/// [api.FeatureSchema]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureDType {
    /// 32-bit float.
    F32,
    /// 64-bit float.
    F64,
    /// 64-bit signed integer.
    I64,
    /// Boolean.
    Bool,
}

// ===========================================================================
// FeatureSpec
// ===========================================================================

/// Specification of a single feature column. Per `specs/training.toml
/// [api.FeatureSchema]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureSpec {
    /// Column name.
    pub name: String,
    /// Element dtype.
    pub dtype: FeatureDType,
    /// Per-axis shape (empty for scalars).
    pub shape: Vec<usize>,
}

// ===========================================================================
// NormalizationSpec
// ===========================================================================

/// Normalisation strategy applied to a feature column. Per
/// `specs/training.toml [api.FeatureSchema]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationSpec {
    /// No normalisation.
    None,
    /// Standard (z-score) normalisation.
    Standard,
    /// Min-max scaling to `[0,1]`.
    MinMax,
    /// Custom named normalisation.
    Custom(String),
}

// ===========================================================================
// FeatureSchema
// ===========================================================================

/// Schema describing the features of a `Dataset` or `TrainedModel`. Per
/// `specs/training.toml [api.FeatureSchema]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureSchema {
    /// One spec per feature column.
    pub features: Vec<FeatureSpec>,
    /// Normalisation applied to the features.
    pub normalization: NormalizationSpec,
}

// ===========================================================================
// ValidationMetrics
// ===========================================================================

/// Metrics from the validation pass of a trained model. Per
/// `specs/training.toml [api.ValidationMetrics]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationMetrics {
    /// Final validation loss.
    pub loss: f64,
    /// Optional top-1 accuracy.
    pub accuracy: Option<f64>,
    /// Optional named custom metrics.
    pub custom: BTreeMap<String, f64>,
}

// ===========================================================================
// TrainedModel
// ===========================================================================

/// The model produced by a `Trainer`. Handed to
/// `themql-artifact::ArtifactWriter` which serialises it into a
/// `ModelArtifact` with hash + metadata. Per `specs/training.toml
/// [api.TrainedModel]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainedModel {
    /// Serialised model bytes (`TorchScript` / `safetensors` / `onnx`).
    pub model_bytes: Vec<u8>,
    /// Serialisation format of `model_bytes`.
    pub format: ModelFormat,
    /// Feature schema the model was trained against.
    pub feature_schema: FeatureSchema,
    /// Validation metrics from the final epoch.
    pub validation_metrics: ValidationMetrics,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_format_serialises_snake_case() {
        let json = serde_json::to_string(&ModelFormat::TorchScript).unwrap();
        assert_eq!(json, "\"torch_script\"");
        let json = serde_json::to_string(&ModelFormat::SafeTensors).unwrap();
        assert_eq!(json, "\"safe_tensors\"");
        let json = serde_json::to_string(&ModelFormat::Onnx).unwrap();
        assert_eq!(json, "\"onnx\"");
    }

    #[test]
    fn feature_dtype_serialises_lowercase() {
        let json = serde_json::to_string(&FeatureDType::F32).unwrap();
        assert_eq!(json, "\"f32\"");
        let json = serde_json::to_string(&FeatureDType::Bool).unwrap();
        assert_eq!(json, "\"bool\"");
    }

    #[test]
    fn feature_schema_round_trips() {
        let schema = FeatureSchema {
            features: vec![FeatureSpec {
                name: "x".to_owned(),
                dtype: FeatureDType::F32,
                shape: vec![],
            }],
            normalization: NormalizationSpec::Standard,
        };
        let json = serde_json::to_string(&schema).unwrap();
        let back: FeatureSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(schema, back);
    }

    #[test]
    fn normalization_spec_custom_round_trips() {
        let n = NormalizationSpec::Custom("log_scale".to_owned());
        let json = serde_json::to_string(&n).unwrap();
        let back: NormalizationSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(n, back);
    }

    #[test]
    fn trained_model_round_trips() {
        let model = TrainedModel {
            model_bytes: vec![1, 2, 3],
            format: ModelFormat::SafeTensors,
            feature_schema: FeatureSchema {
                features: vec![FeatureSpec {
                    name: "state".to_owned(),
                    dtype: FeatureDType::F64,
                    shape: vec![21],
                }],
                normalization: NormalizationSpec::None,
            },
            validation_metrics: ValidationMetrics {
                loss: 0.1,
                accuracy: Some(0.95),
                custom: BTreeMap::new(),
            },
        };
        let json = serde_json::to_string(&model).unwrap();
        let back: TrainedModel = serde_json::from_str(&json).unwrap();
        assert_eq!(model, back);
    }

    #[test]
    fn validation_metrics_custom_round_trips() {
        let mut custom = BTreeMap::new();
        custom.insert("f1".to_owned(), 0.8);
        let m = ValidationMetrics {
            loss: 0.5,
            accuracy: None,
            custom,
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: ValidationMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }
}
