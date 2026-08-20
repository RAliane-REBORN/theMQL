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

use std::future::Future;

use serde::{Deserialize, Serialize};

use themql_core::{Error, ErrorCode};

pub use themql_artifact::ArtifactMetadata;
pub use themql_schema::{
    FeatureDType, FeatureSchema, FeatureSpec, ModelFormat, NormalizationSpec, TrainedModel,
    ValidationMetrics,
};

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
// TchTrainer — tch-backed trainer (behind `tch-backend` feature)
// ===========================================================================

/// tch-backed [`Trainer`]. Performs a placeholder training loop that
/// proves tch compiles and can create tensors. Per `specs/training.toml`,
/// tch is the primary tensor framework on desktop. Real training is
/// deferred.
#[cfg(feature = "tch-backend")]
pub struct TchTrainer {
    kind: TrainerKind,
}

#[cfg(feature = "tch-backend")]
impl TchTrainer {
    /// Construct a new `TchTrainer` for the given trainer `kind`.
    #[must_use]
    pub fn new(kind: TrainerKind) -> Self {
        Self { kind }
    }
}

#[cfg(feature = "tch-backend")]
impl Trainer for TchTrainer {
    fn train(
        &self,
        dataset: &Dataset,
        _config: &TrainingConfig,
    ) -> impl Future<Output = Result<TrainedModel, TrainingError>> {
        use std::collections::BTreeMap;
        let n_rows = dataset.features.len();
        let n_cols = dataset.features.first().map(|r| r.len()).unwrap_or(0);
        let res = (|| -> Result<TrainedModel, TrainingError> {
            if n_rows == 0 || n_cols == 0 {
                return Err(TrainingError::DatasetInvalid("empty dataset".to_owned()));
            }
            let rows_i64 = i64::try_from(n_rows)
                .map_err(|_| TrainingError::DatasetInvalid("row count overflow".to_owned()))?;
            let cols_i64 = i64::try_from(n_cols)
                .map_err(|_| TrainingError::DatasetInvalid("col count overflow".to_owned()))?;
            let flat: Vec<f32> = dataset
                .features
                .iter()
                .flat_map(|r| r.iter().map(|v| *v as f32))
                .collect();
            let _t = tch::Tensor::from_slice(&flat)
                .to_kind(tch::Kind::Float)
                .reshape([rows_i64, cols_i64]);
            let zeros = tch::Tensor::zeros(
                [rows_i64, cols_i64.max(1)],
                (tch::Kind::Float, tch::Device::Cpu),
            );
            let _out = zeros.tanh();
            let model_bytes: Vec<u8> = (0u32..16).flat_map(|i| i.to_le_bytes()).collect();
            let feature_schema = FeatureSchema {
                features: (0..n_cols)
                    .map(|i| FeatureSpec {
                        name: format!("f{i}"),
                        dtype: FeatureDType::F32,
                        shape: vec![],
                    })
                    .collect(),
                normalization: NormalizationSpec::None,
            };
            Ok(TrainedModel {
                model_bytes,
                format: ModelFormat::TorchScript,
                feature_schema,
                validation_metrics: ValidationMetrics {
                    loss: 0.0,
                    accuracy: None,
                    custom: BTreeMap::new(),
                },
            })
        })();
        std::future::ready(res)
    }

    fn kind(&self) -> TrainerKind {
        self.kind
    }
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

    #[cfg(feature = "tch-backend")]
    mod tch_backend {
        #![allow(clippy::unwrap_used, clippy::expect_used)]
        use super::*;

        #[test]
        fn tch_tensor_creation() {
            let t = tch::Tensor::zeros(&[10, 5], (tch::Kind::Float, tch::Device::Cpu));
            assert_eq!(t.size(), vec![10, 5]);
            assert!(t.sum(tch::Kind::Float).double_value(&[]).abs() < 1e-12);
        }

        #[test]
        fn tch_trainer_train_produces_non_empty_model_bytes() {
            let trainer = TchTrainer::new(TrainerKind::Dense);
            let dataset = Dataset {
                features: vec![vec![1.0, 2.0], vec![3.0, 4.0]],
                labels: vec![vec![0.0], vec![1.0]],
                feature_schema: FeatureSchema {
                    features: vec![
                        FeatureSpec {
                            name: "x".to_owned(),
                            dtype: FeatureDType::F32,
                            shape: vec![],
                        },
                        FeatureSpec {
                            name: "y".to_owned(),
                            dtype: FeatureDType::F32,
                            shape: vec![],
                        },
                    ],
                    normalization: NormalizationSpec::None,
                },
            };
            let cfg = TrainingConfig {
                kind: TrainerKind::Dense,
                epochs: 1,
                batch_size: 2,
                learning_rate: 1e-3,
                weight_decay: None,
                early_stopping: None,
                pruning: None,
                sparsification: None,
            };
            let model = futures_block_on(trainer.train(&dataset, &cfg)).expect("train");
            assert!(!model.model_bytes.is_empty());
            assert_eq!(model.format, ModelFormat::TorchScript);
        }

        #[test]
        fn tch_trainer_rejects_empty_dataset() {
            let trainer = TchTrainer::new(TrainerKind::Dense);
            let dataset = Dataset {
                features: vec![],
                labels: vec![],
                feature_schema: FeatureSchema {
                    features: vec![],
                    normalization: NormalizationSpec::None,
                },
            };
            let cfg = TrainingConfig {
                kind: TrainerKind::Dense,
                epochs: 1,
                batch_size: 2,
                learning_rate: 1e-3,
                weight_decay: None,
                early_stopping: None,
                pruning: None,
                sparsification: None,
            };
            assert!(futures_block_on(trainer.train(&dataset, &cfg)).is_err());
        }

        fn futures_block_on<F: std::future::Future>(fut: F) -> F::Output {
            use std::future::Future;
            use std::pin::Pin;
            use std::sync::Arc;
            use std::task::{Context, Poll, Wake, Waker};

            struct NoopWake;
            impl Wake for NoopWake {
                fn wake(self: Arc<Self>) {}
            }
            let waker = Waker::from(Arc::new(NoopWake));
            let mut cx = Context::from_waker(&waker);
            let mut fut = Box::pin(fut);
            loop {
                if let Poll::Ready(v) = Future::poll(Pin::as_mut(&mut fut), &mut cx) {
                    return v;
                }
            }
        }
    }
}
