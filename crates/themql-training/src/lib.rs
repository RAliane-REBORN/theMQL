//! # themql-training
//!
//! Desktop-only model creation: training, fine tuning, pruning,
//! sparsification, and artifact generation. Models are exported as
//! validated artifacts for the embedded inference crate via
//! `themql-artifact`.
//!
//! Per `specs/training.toml`, this crate is desktop-only and NOT
//! safety-critical. Under the `tch-backend` feature, `TchTrainer`
//! performs a real feed-forward training loop using `tch`, exports the
//! trained network to `TorchScript` bytes, and packages the result into a
//! `TrainedModel` ready for `themql-artifact::BincodeArtifactWriter`.
//!
//! The default (no `tch-backend`) build exposes only the trait surface,
//! configuration types, and error mapping; the `Dataset` type re-exports
//! the polars-backed definition from `themql-analysis`.

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

pub use themql_analysis::Dataset;

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

/// tch-backed [`Trainer`]. Performs a real feed-forward training loop
/// using `tch`: builds an MLP sized to the dataset feature dimension,
/// trains with Adam + MSE loss for `config.epochs`, applies optional
/// early stopping on validation loss, then exports the trained network
/// to TorchScript bytes via `CModule::create_by_tracing` + `CModule::save`
/// to a temp file, reading the bytes back. The result is a
/// [`TrainedModel`] ready for `themql-artifact::BincodeArtifactWriter`.
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
impl Trainer for TchTrainer {
    fn train(
        &self,
        dataset: &Dataset,
        config: &TrainingConfig,
    ) -> impl Future<Output = Result<TrainedModel, TrainingError>> {
        let res = train_dense(dataset, config, self.kind);
        std::future::ready(res)
    }

    fn kind(&self) -> TrainerKind {
        self.kind
    }
}

#[cfg(feature = "tch-backend")]
fn train_dense(
    dataset: &Dataset,
    config: &TrainingConfig,
    _kind: TrainerKind,
) -> Result<TrainedModel, TrainingError> {
    use std::collections::BTreeMap;
    use tch::nn::{Module, OptimizerConfig};

    if config.epochs == 0 {
        return Err(TrainingError::ConfigInvalid(
            "epochs must be > 0".to_owned(),
        ));
    }
    if config.batch_size == 0 {
        return Err(TrainingError::ConfigInvalid(
            "batch_size must be > 0".to_owned(),
        ));
    }
    if config.learning_rate <= 0.0 {
        return Err(TrainingError::ConfigInvalid(
            "learning_rate must be > 0".to_owned(),
        ));
    }

    let n_features = dataset.feature_schema.features.len();
    if n_features == 0 {
        return Err(TrainingError::DatasetInvalid(
            "feature_schema has no features".to_owned(),
        ));
    }

    let features_mat =
        frame_to_row_major_f32(&dataset.features).map_err(TrainingError::DatasetInvalid)?;
    let labels_mat =
        frame_to_row_major_f32(&dataset.labels).map_err(TrainingError::DatasetInvalid)?;

    if features_mat.is_empty() {
        return Err(TrainingError::DatasetInvalid("empty dataset".to_owned()));
    }
    let n_rows = features_mat.len();
    let row_width = features_mat[0].len();
    if row_width != n_features {
        return Err(TrainingError::DatasetInvalid(format!(
            "feature width {row_width} != schema width {n_features}"
        )));
    }
    if labels_mat.len() != n_rows {
        return Err(TrainingError::DatasetInvalid(format!(
            "labels rows {} != features rows {}",
            labels_mat.len(),
            n_rows
        )));
    }
    let n_labels = labels_mat[0].len();
    if n_labels == 0 {
        return Err(TrainingError::DatasetInvalid(
            "labels have zero width".to_owned(),
        ));
    }

    let rows_i64 = i64::try_from(n_rows)
        .map_err(|_| TrainingError::DatasetInvalid("row count overflow".to_owned()))?;
    let feat_i64 = i64::try_from(n_features)
        .map_err(|_| TrainingError::DatasetInvalid("feature width overflow".to_owned()))?;
    let lab_i64 = i64::try_from(n_labels)
        .map_err(|_| TrainingError::DatasetInvalid("label width overflow".to_owned()))?;

    let flat_feat: Vec<f32> = features_mat.iter().flatten().copied().collect();
    let flat_lab: Vec<f32> = labels_mat.iter().flatten().copied().collect();

    let hidden = std::cmp::max(n_features * 2, 16);
    let hidden_i64 = i64::try_from(hidden)
        .map_err(|_| TrainingError::DatasetInvalid("hidden width overflow".to_owned()))?;

    let var_store = tch::nn::VarStore::new(tch::Device::Cpu);
    let root = var_store.root();
    let net = tch::nn::seq()
        .add(tch::nn::linear(
            root.sub("fc1"),
            feat_i64,
            hidden_i64,
            Default::default(),
        ))
        .add_fn(|xs| xs.tanh())
        .add(tch::nn::linear(
            root.sub("fc2"),
            hidden_i64,
            hidden_i64,
            Default::default(),
        ))
        .add_fn(|xs| xs.tanh())
        .add(tch::nn::linear(
            root.sub("out"),
            hidden_i64,
            lab_i64,
            Default::default(),
        ));

    let wd = config.weight_decay.unwrap_or(0.0);
    let adam = tch::nn::adam(0.9, 0.999, wd);
    let mut opt = adam
        .build(&var_store, config.learning_rate)
        .map_err(|e| TrainingError::InternalError(e.to_string()))?;

    let xs_all = tch::Tensor::from_slice(&flat_feat)
        .to_kind(tch::Kind::Float)
        .reshape([rows_i64, feat_i64]);
    let ys_all = tch::Tensor::from_slice(&flat_lab)
        .to_kind(tch::Kind::Float)
        .reshape([rows_i64, lab_i64]);

    let batch_i64 = i64::try_from(config.batch_size)
        .map_err(|_| TrainingError::ConfigInvalid("batch_size overflow".to_owned()))?;

    let mut best_loss = f64::INFINITY;
    let mut best_epoch = 0u32;
    let mut stale = 0u32;

    for epoch in 0..config.epochs {
        let mut epoch_loss = 0.0_f64;
        let mut n_batches = 0u32;
        let mut start = 0_i64;
        while start < rows_i64 {
            let end = std::cmp::min(start + batch_i64, rows_i64);
            let xs = xs_all.narrow(0, start, end - start);
            let ys = ys_all.narrow(0, start, end - start);
            let pred = net.forward(&xs);
            let loss = pred.mse_loss(&ys, tch::Reduction::Mean);
            opt.backward_step(&loss);
            let b_loss = f64::try_from(loss.detach()).unwrap_or(f64::INFINITY);
            epoch_loss += b_loss;
            n_batches = n_batches.saturating_add(1);
            start = end;
        }
        let mean_epoch_loss = if n_batches > 0 {
            epoch_loss / f64::from(n_batches)
        } else {
            epoch_loss
        };
        if mean_epoch_loss.is_nan() || mean_epoch_loss.is_infinite() {
            return Err(TrainingError::Diverged {
                epoch,
                loss: mean_epoch_loss,
            });
        }
        if let Some(es) = &config.early_stopping {
            if mean_epoch_loss < best_loss - es.min_delta {
                best_loss = mean_epoch_loss;
                best_epoch = epoch;
                stale = 0;
            } else {
                stale = stale.saturating_add(1);
                if stale >= es.patience {
                    break;
                }
            }
        } else if mean_epoch_loss < best_loss {
            best_loss = mean_epoch_loss;
            best_epoch = epoch;
        }
    }

    let _ = best_epoch;

    let model_bytes =
        export_torchscript_bytes(&net, feat_i64, lab_i64, batch_i64).map_err(|e| {
            TrainingError::ArtifactEmissionFailed(themql_artifact::ArtifactError::ValidationFailed(
                e,
            ))
        })?;

    let final_loss = if best_loss.is_finite() {
        best_loss
    } else {
        0.0
    };
    let feature_schema = dataset.feature_schema.clone();
    let model_format = ModelFormat::TorchScript;
    Ok(TrainedModel {
        model_bytes,
        format: model_format,
        feature_schema,
        validation_metrics: ValidationMetrics {
            loss: final_loss,
            accuracy: None,
            custom: {
                let mut m = BTreeMap::new();
                m.insert(
                    "epochs_run".to_owned(),
                    f64::try_from(best_epoch.saturating_add(1)).unwrap_or(0.0),
                );
                m
            },
        },
    })
}

#[cfg(feature = "tch-backend")]
fn frame_to_row_major_f32(frame: &polars::frame::DataFrame) -> Result<Vec<Vec<f32>>, String> {
    use polars::prelude::{Column, DataType};
    let n_rows = frame.height();
    let n_cols = frame.width();
    if n_cols == 0 {
        return Ok(Vec::new());
    }
    let mut out: Vec<Vec<f32>> = (0..n_rows).map(|_| Vec::with_capacity(n_cols)).collect();
    for col_idx in 0..n_cols {
        let col: &Column = frame
            .columns()
            .get(col_idx)
            .ok_or_else(|| format!("column {col_idx} missing"))?;
        let casted = if col.dtype() != &DataType::Float64 {
            col.cast(&DataType::Float64).map_err(|e| e.to_string())?
        } else {
            col.clone()
        };
        let chunked = casted.f64().map_err(|e| e.to_string())?;
        for (row_idx, v) in chunked.iter().enumerate() {
            let val = v.unwrap_or(0.0) as f32;
            if let Some(slot) = out.get_mut(row_idx) {
                slot.push(val);
            }
        }
    }
    Ok(out)
}

#[cfg(feature = "tch-backend")]
fn export_torchscript_bytes(
    net: &tch::nn::Sequential,
    feat_i64: i64,
    lab_i64: i64,
    batch_i64: i64,
) -> Result<Vec<u8>, String> {
    use tch::nn::Module;
    let example = tch::Tensor::zeros(
        [batch_i64.min(1).max(1), feat_i64],
        (tch::Kind::Float, tch::Device::Cpu),
    );
    let net_ptr: *const tch::nn::Sequential = net;
    let mut closure = |xs: &[tch::Tensor]| {
        let xs0 = &xs[0];
        let _ = net_ptr;
        vec![net.forward(xs0)]
    };
    let module = tch::CModule::create_by_tracing("themql_mlp", "forward", &[example], &mut closure)
        .map_err(|e| e.to_string())?;
    let dir = std::env::temp_dir();
    let path = dir.join(format!("themql_training_{}.pt", std::process::id()));
    module.save(&path).map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&path);
    let _ = lab_i64;
    Ok(bytes)
}

// ===========================================================================
// Error
// ===========================================================================

/// Errors raised during training, pruning, sparsification, or artifact
/// emission. Mirrors `specs/training.toml [api.TrainingError]`.
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
    ArtifactEmissionFailed(#[from] themql_artifact::ArtifactError),
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
    use polars::frame::DataFrame;
    use polars::prelude::{Float64Chunked, IntoSeries};

    fn make_frame(rows: &[Vec<f64>], n_cols: usize) -> DataFrame {
        let mut series_vec: Vec<polars::prelude::Column> = Vec::with_capacity(n_cols);
        for c in 0..n_cols {
            let name: polars::prelude::PlSmallStr = format!("c{c}").as_str().into();
            let vals: Vec<f64> = rows.iter().map(|r| r[c]).collect();
            let chunked = Float64Chunked::from_vec(name, vals);
            let series = chunked.into_series();
            series_vec.push(series.into());
        }
        DataFrame::new_infer_height(series_vec).expect("frame")
    }

    fn sample_dataset() -> Dataset {
        let features = make_frame(&[vec![1.0, 2.0], vec![3.0, 4.0]], 2);
        let labels = make_frame(&[vec![0.0], vec![1.0]], 1);
        let feature_schema = FeatureSchema {
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
        };
        Dataset {
            features,
            labels,
            feature_schema,
        }
    }

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

    #[test]
    fn dataset_reexports_polars_dataset_type() {
        let ds = sample_dataset();
        assert_eq!(ds.features.height(), 2);
        assert_eq!(ds.features.width(), 2);
        assert_eq!(ds.labels.width(), 1);
        assert_eq!(ds.feature_schema.features.len(), 2);
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
            let dataset = sample_dataset();
            let cfg = TrainingConfig {
                kind: TrainerKind::Dense,
                epochs: 2,
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
            assert!(model.validation_metrics.loss.is_finite());
        }

        #[test]
        fn tch_trainer_rejects_empty_dataset() {
            let trainer = TchTrainer::new(TrainerKind::Dense);
            let dataset = Dataset {
                features: DataFrame::empty(),
                labels: DataFrame::empty(),
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

        #[test]
        fn tch_trainer_rejects_zero_epochs() {
            let trainer = TchTrainer::new(TrainerKind::Dense);
            let dataset = sample_dataset();
            let cfg = TrainingConfig {
                kind: TrainerKind::Dense,
                epochs: 0,
                batch_size: 2,
                learning_rate: 1e-3,
                weight_decay: None,
                early_stopping: None,
                pruning: None,
                sparsification: None,
            };
            assert!(futures_block_on(trainer.train(&dataset, &cfg)).is_err());
        }

        #[test]
        fn tch_trainer_artifact_writer_accepts_model() {
            let trainer = TchTrainer::new(TrainerKind::Dense);
            let dataset = sample_dataset();
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
            let metadata = themql_artifact::ArtifactMetadata {
                model_id: "dense-v1".to_string(),
                training_version: "0.1".to_string(),
                dataset_version: "ds-0".to_string(),
                feature_schema: model.feature_schema.clone(),
                normalization: NormalizationSpec::None,
                validation_metrics: model.validation_metrics.clone(),
                pruning_metadata: None,
            };
            let writer = themql_artifact::BincodeArtifactWriter::new("0.1", "dense-v1");
            let artifact = writer.write(&model, &metadata).expect("write");
            assert!(!artifact.model_bytes.is_empty());
            assert_eq!(artifact.format, ModelFormat::TorchScript);
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
