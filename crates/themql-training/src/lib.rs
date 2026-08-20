//! # themql-training
//!
//! Desktop-only model creation: training, fine tuning, pruning,
//! sparsification, and artifact generation. Models are exported as
//! validated artifacts for the embedded inference crate via
//! `themql-artifact`.
//!
//! Per `specs/training.toml`, this crate is desktop-only and NOT
//! safety-critical. The actual tensor framework (`tch`) and dataframe
//! library (`polars`) are deferred to a future task; this module
//! defines the trait surface, configuration types, and error mapping
//! only.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::collections::BTreeMap;
use std::future::Future;

use serde::{Deserialize, Serialize};

use themql_core::{Error, ErrorCode};

// ===========================================================================
// TrainerKind
// ===========================================================================

/// Variant of trainer. Mirrors `specs/training.toml [api.TrainerKind]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainerKind {
    /// Dense feed-forward / dense network training.
    Dense,
    /// Physics-informed neural network training.
    Pinn,
    /// Gradient-boosted tree training.
    GradientBoosting,
    /// Fine tuning of an existing model.
    FineTuning,
}

// ===========================================================================
// Feature schema (defined locally; re-exportable from themql-artifact
// when that crate grows a real implementation)
// ===========================================================================

/// Element type of a feature column.
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

/// Specification of a single feature column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureSpec {
    /// Column name.
    pub name: String,
    /// Element dtype.
    pub dtype: FeatureDType,
    /// Per-axis shape (empty for scalars).
    pub shape: Vec<usize>,
}

/// Normalisation strategy applied to a feature column.
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

/// Schema describing the features of a `Dataset` or `TrainedModel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureSchema {
    /// One spec per feature column.
    pub features: Vec<FeatureSpec>,
    /// Normalisation applied to the features.
    pub normalization: NormalizationSpec,
}

// ===========================================================================
// Dataset (minimal — no polars dependency)
// ===========================================================================

/// Training dataset. Per `specs/training.toml [api.Dataset]`, the
/// canonical shape is a polars `DataFrame`; this minimal stand-in holds
/// the same data as `Vec<Vec<f64>>` until polars is wired in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    /// Feature matrix: one row per sample, one column per feature.
    pub features: Vec<Vec<f64>>,
    /// Label matrix: one row per sample, one column per label.
    pub labels: Vec<Vec<f64>>,
    /// Schema for the feature columns.
    pub feature_schema: FeatureSchema,
}

// ===========================================================================
// TrainingConfig + early stopping + pruning + sparsification
// ===========================================================================

/// Early-stopping configuration. Training halts if the watched metric
/// fails to improve by at least `min_delta` for `patience` consecutive
/// epochs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarlyStoppingConfig {
    /// Number of epochs without improvement before stopping.
    pub patience: u32,
    /// Minimum improvement in the watched metric to count as progress.
    pub min_delta: f64,
    /// Name of the metric to watch (e.g. `"val_loss"`).
    pub metric: String,
}

/// Strategy for pruning weights from a trained model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PruningStrategy {
    /// Magnitude pruning: zero the smallest-magnitude weights.
    Magnitude,
    /// Structured pruning: remove entire channels/heads.
    Structured,
    /// Lottery-ticket pruning: re-train the pruned mask from init.
    LotteryTicket,
}

/// Pruning configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PruningConfig {
    /// Pruning strategy.
    pub strategy: PruningStrategy,
    /// Target sparsity in `[0,1]`.
    pub target_sparsity: f32,
}

/// Strategy for sparsifying / compressing a trained model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SparsificationStrategy {
    /// Quantise weights to `bits`-bit integers.
    Quantization {
        /// Bit width of the quantised weights (e.g. 8).
        bits: u8,
    },
    /// Knowledge distillation into a smaller student model.
    Distillation,
}

/// Sparsification configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SparsificationConfig {
    /// Sparsification strategy.
    pub strategy: SparsificationStrategy,
    /// Target sparsity in `[0,1]`.
    pub target_sparsity: f32,
}

/// Top-level training configuration. Mirrors
/// `specs/training.toml [api.TrainingConfig]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Trainer variant to run.
    pub kind: TrainerKind,
    /// Number of training epochs.
    pub epochs: u32,
    /// Mini-batch size.
    pub batch_size: u32,
    /// Learning rate.
    pub learning_rate: f64,
    /// Optional L2 weight decay.
    pub weight_decay: Option<f64>,
    /// Optional early-stopping policy.
    pub early_stopping: Option<EarlyStoppingConfig>,
    /// Optional pruning policy applied after training.
    pub pruning: Option<PruningConfig>,
    /// Optional sparsification policy applied after training.
    pub sparsification: Option<SparsificationConfig>,
}

// ===========================================================================
// TrainedModel + ModelFormat + ValidationMetrics
// ===========================================================================

/// Serialisation format of the trained model bytes.
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

/// Metrics from the validation pass of a trained model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationMetrics {
    /// Final validation loss.
    pub loss: f64,
    /// Optional top-1 accuracy.
    pub accuracy: Option<f64>,
    /// Optional named custom metrics.
    pub custom: BTreeMap<String, f64>,
}

/// The model produced by a [`Trainer`]. Handed to
/// `themql-artifact::ArtifactWriter` for validation + serialisation.
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
// Trainer trait
// ===========================================================================

/// Top-level training trait. `train()` consumes a [`Dataset`] and a
/// [`TrainingConfig`] and produces a [`TrainedModel`]. The model is then
/// handed to `themql-artifact` for validation + serialisation.
#[allow(async_fn_in_trait)]
pub trait Trainer: Send + Sync {
    /// Train a model on `dataset` using `config`.
    ///
    /// # Errors
    /// Returns [`TrainingError`] on any training failure.
    fn train(
        &self,
        dataset: &Dataset,
        config: &TrainingConfig,
    ) -> impl Future<Output = Result<TrainedModel, TrainingError>>;

    /// Returns the trainer variant.
    fn kind(&self) -> TrainerKind;
}

// ===========================================================================
// Error
// ===========================================================================

/// Errors raised during training, pruning, sparsification, or artifact
/// emission. Mirrors `specs/training.toml [api.TrainingError]`; the
/// `ArtifactEmissionFailed` variant carries a `String` rather than an
/// `ArtifactError` because `themql-artifact` does not yet export that
/// type.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TrainingError {
    /// The dataset was malformed or empty.
    #[error("dataset invalid: {0}")]
    DatasetInvalid(String),
    /// The training configuration was invalid.
    #[error("config invalid: {0}")]
    ConfigInvalid(String),
    /// Training diverged at the given epoch with the given loss.
    #[error("training diverged at epoch {epoch} (loss={loss})")]
    Diverged {
        /// Epoch at which divergence was detected.
        epoch: u32,
        /// Loss value at the divergent epoch.
        loss: f64,
    },
    /// Pruning failed.
    #[error("pruning failed: {0}")]
    PruningFailed(String),
    /// Sparsification failed.
    #[error("sparsification failed: {0}")]
    SparsificationFailed(String),
    /// Artifact emission failed.
    #[error("artifact emission failed: {0}")]
    ArtifactEmissionFailed(String),
    /// Unexpected internal failure.
    #[error("internal error: {0}")]
    InternalError(String),
}

impl From<TrainingError> for Error {
    fn from(e: TrainingError) -> Self {
        let msg = e.to_string();
        match e {
            TrainingError::DatasetInvalid(_) | TrainingError::ConfigInvalid(_) => {
                Error::validation_error(msg)
            }
            TrainingError::Diverged { .. }
            | TrainingError::PruningFailed(_)
            | TrainingError::SparsificationFailed(_)
            | TrainingError::ArtifactEmissionFailed(_)
            | TrainingError::InternalError(_) => Error::new(ErrorCode::InternalError, msg),
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn training_config_construction() {
        let cfg = TrainingConfig {
            kind: TrainerKind::Dense,
            epochs: 100,
            batch_size: 32,
            learning_rate: 1e-3,
            weight_decay: Some(1e-4),
            early_stopping: Some(EarlyStoppingConfig {
                patience: 10,
                min_delta: 1e-4,
                metric: "val_loss".to_owned(),
            }),
            pruning: Some(PruningConfig {
                strategy: PruningStrategy::Magnitude,
                target_sparsity: 0.5,
            }),
            sparsification: Some(SparsificationConfig {
                strategy: SparsificationStrategy::Quantization { bits: 8 },
                target_sparsity: 0.5,
            }),
        };
        assert_eq!(cfg.kind, TrainerKind::Dense);
        assert_eq!(cfg.epochs, 100);
        assert_eq!(cfg.batch_size, 32);
        assert!(cfg.weight_decay.is_some());
        assert!(cfg.early_stopping.is_some());
        assert!(cfg.pruning.is_some());
        assert!(cfg.sparsification.is_some());
    }

    #[test]
    fn trainer_kind_variants() {
        let kinds = [
            TrainerKind::Dense,
            TrainerKind::Pinn,
            TrainerKind::GradientBoosting,
            TrainerKind::FineTuning,
        ];
        for (i, k) in kinds.iter().enumerate() {
            assert_eq!(*k, kinds[i]);
        }
        let json = serde_json::to_string(&TrainerKind::Pinn).unwrap();
        assert_eq!(json, "\"pinn\"");
    }

    #[test]
    fn training_error_converts_to_core_error() {
        let e: Error = TrainingError::Diverged {
            epoch: 42,
            loss: 1e9,
        }
        .into();
        assert_eq!(e.code, ErrorCode::InternalError);

        let e2: Error = TrainingError::ConfigInvalid("bad".to_owned()).into();
        assert_eq!(e2.code, ErrorCode::ValidationError);
    }

    #[test]
    fn feature_schema_serialises() {
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
    fn sparsification_strategy_quantization_round_trips() {
        let s = SparsificationStrategy::Quantization { bits: 8 };
        let json = serde_json::to_string(&s).unwrap();
        let back: SparsificationStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
