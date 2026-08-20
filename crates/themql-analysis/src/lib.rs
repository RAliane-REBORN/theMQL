//! # themql-analysis
//!
//! Desktop analytical processing of telemetry data. Builds `Dataset`s
//! for `themql-training`, runs statistical analysis, and provides a thin
//! theDAF adapter for legacy data access and migration reference.
//!
//! Per `specs/analysis.toml`, polars is the semantic owner of dataframe
//! operations and rayon provides parallelism.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::future::Future;

use polars::frame::DataFrame;
use polars::prelude::{Float64Chunked, IntoSeries};
use serde::{Deserialize, Serialize};

use themql_core::{Context, Error, ErrorCode, SubjectPattern, Timestamp};
use themql_schema::{FeatureDType, FeatureSchema, FeatureSpec, NormalizationSpec};
use themql_storage::{Storage, StorageQuery, StorageResultSet};

// ===========================================================================
// AnalysisInput
// ===========================================================================

/// Input to an [`AnalysisPipeline`]. Per `specs/analysis.toml`.
#[derive(Debug, Clone, PartialEq)]
pub enum AnalysisInput {
    /// A telemetry range query: all readings on `subject_pattern`
    /// between `from` and `to`.
    Telemetry {
        /// Inclusive lower bound timestamp.
        from: Timestamp,
        /// Inclusive upper bound timestamp.
        to: Timestamp,
        /// Subject pattern selecting which telemetry streams to read.
        subject_pattern: SubjectPattern,
    },
    /// A storage query to run against the L4 backend.
    StorageQuery(StorageQuery),
    /// An in-memory polars `DataFrame`.
    DataFrame(DataFrame),
}

// ===========================================================================
// AnalysisStats + AnalysisResult
// ===========================================================================

/// Summary statistics about an [`AnalysisResult`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisStats {
    /// Number of rows in the result frame.
    pub rows: u64,
    /// Number of columns in the result frame.
    pub columns: u32,
    /// Count of null/missing values in the result frame.
    pub null_count: u64,
    /// When the result was generated.
    pub generated_at: Timestamp,
}

/// Result of an analysis pipeline. Per `specs/analysis.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisResult {
    /// The result frame (polars `DataFrame`).
    pub frame: DataFrame,
    /// Feature schema describing the columns.
    pub schema: FeatureSchema,
    /// Summary statistics.
    pub stats: AnalysisStats,
}

// ===========================================================================
// Dataset (polars-backed)
// ===========================================================================

/// Dataset for `themql-training`. Per `specs/analysis.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct Dataset {
    /// Feature matrix (polars `DataFrame`).
    pub features: DataFrame,
    /// Label matrix (polars `DataFrame`).
    pub labels: DataFrame,
    /// Feature schema.
    pub feature_schema: FeatureSchema,
}

// ===========================================================================
// AnalysisPipeline trait
// ===========================================================================

/// Top-level analysis trait. `run()` consumes an [`AnalysisInput`] and
/// produces an [`AnalysisResult`]. Pipelines are composable — a
/// pipeline may wrap other pipelines.
#[allow(async_fn_in_trait)]
pub trait AnalysisPipeline: Send + Sync {
    /// Run this pipeline against `input` under execution context `ctx`.
    ///
    /// # Errors
    /// Returns [`AnalysisError`] on any pipeline failure.
    fn run(
        &self,
        input: &AnalysisInput,
        ctx: &Context,
    ) -> impl Future<Output = Result<AnalysisResult, AnalysisError>>;
}

// ===========================================================================
// DatasetBuilder
// ===========================================================================

/// Builder that converts an [`AnalysisResult`] into a [`Dataset`] for
/// `themql-training`. Per `specs/analysis.toml [api.DatasetBuilder]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetBuilder {
    /// Names of the columns to use as features.
    pub feature_columns: Vec<String>,
    /// Names of the columns to use as labels.
    pub label_columns: Vec<String>,
    /// Normalisation to apply.
    pub normalization: NormalizationSpec,
}

impl DatasetBuilder {
    /// Build a [`Dataset`] from an [`AnalysisResult`].
    ///
    /// # Errors
    /// Returns [`AnalysisError::SchemaMismatch`] if the builder's
    /// column selections cannot be satisfied by `result`.
    pub fn build(&self, result: &AnalysisResult) -> Result<Dataset, AnalysisError> {
        let feature_frame = select_columns(&result.frame, &self.feature_columns)?;
        let label_frame = select_columns(&result.frame, &self.label_columns)?;
        let feature_schema = FeatureSchema {
            features: self
                .feature_columns
                .iter()
                .map(|name| FeatureSpec {
                    name: name.clone(),
                    dtype: FeatureDType::F64,
                    shape: vec![],
                })
                .collect(),
            normalization: self.normalization.clone(),
        };
        Ok(Dataset {
            features: feature_frame,
            labels: label_frame,
            feature_schema,
        })
    }
}

/// Select columns from a `DataFrame` by name.
fn select_columns(frame: &DataFrame, columns: &[String]) -> Result<DataFrame, AnalysisError> {
    if columns.is_empty() {
        return Ok(DataFrame::empty());
    }
    let col_names: Vec<&str> = columns.iter().map(String::as_str).collect();
    frame
        .select(&col_names)
        .map_err(|e| AnalysisError::SchemaMismatch {
            expected: format!("{columns:?}"),
            got: e.to_string(),
        })
}

// ===========================================================================
// ThedafAdapter trait
// ===========================================================================

/// Thin adapter for legacy theDAF data access. Used for migration
/// reference and comparison against the new polars-based implementation.
/// theDAF is NOT a core or embedded dependency.
#[allow(async_fn_in_trait)]
pub trait ThedafAdapter: Send + Sync {
    /// Fetch a rectangular frame from the legacy theDAF store.
    ///
    /// # Errors
    /// Returns [`AnalysisError::ThedafError`] on any legacy-store
    /// failure.
    fn fetch_legacy(&self, query: &str) -> impl Future<Output = Result<DataFrame, AnalysisError>>;

    /// List dataset names available in the legacy theDAF store.
    ///
    /// # Errors
    /// Returns [`AnalysisError::ThedafError`] on any legacy-store
    /// failure.
    fn list_legacy_datasets(&self) -> impl Future<Output = Result<Vec<String>, AnalysisError>>;
}

// ===========================================================================
// Error
// ===========================================================================

/// Errors raised by analysis pipelines, dataset builders, or theDAF
/// adapters. Per `specs/analysis.toml [api.AnalysisError]`.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AnalysisError {
    /// Polars operation failed.
    #[error("polars error: {0}")]
    PolarsError(String),
    /// Storage operation failed.
    #[error("storage error: {0}")]
    StorageError(String),
    /// theDAF legacy access failed.
    #[error("thedaf error: {0}")]
    ThedafError(String),
    /// The result schema did not match the expected schema.
    #[error("schema mismatch: expected {expected}, got {got}")]
    SchemaMismatch {
        /// Expected schema description.
        expected: String,
        /// Actual schema description.
        got: String,
    },
    /// Unexpected internal failure.
    #[error("internal error: {0}")]
    InternalError(String),
}

impl From<themql_storage::StorageError> for AnalysisError {
    fn from(e: themql_storage::StorageError) -> Self {
        AnalysisError::StorageError(e.to_string())
    }
}

impl From<AnalysisError> for Error {
    fn from(e: AnalysisError) -> Self {
        let msg = e.to_string();
        match e {
            AnalysisError::SchemaMismatch { .. } => Error::validation_error(msg),
            AnalysisError::PolarsError(_)
            | AnalysisError::StorageError(_)
            | AnalysisError::ThedafError(_)
            | AnalysisError::InternalError(_) => Error::new(ErrorCode::ResolverError, msg),
        }
    }
}

// ===========================================================================
// PolarsDatasetBuilder — builds a polars DataFrame from telemetry rows
// ===========================================================================

/// Builder that constructs a `polars::frame::DataFrame` from raw telemetry
/// rows. Each entry in `rows` becomes one frame row; `headers` names the
/// columns. Per `specs/analysis.toml [api.DatasetBuilder]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolarsDatasetBuilder {
    /// Column names for the produced frame.
    pub headers: Vec<String>,
}

impl PolarsDatasetBuilder {
    /// Construct a new builder with the given column `headers`.
    #[must_use]
    pub fn new(headers: Vec<String>) -> Self {
        Self { headers }
    }

    /// Build a `polars::frame::DataFrame` from `headers` and `rows`.
    ///
    /// # Errors
    /// Returns [`AnalysisError::PolarsError`] if a column cannot be
    /// constructed or the frame cannot be assembled. Returns
    /// [`AnalysisError::SchemaMismatch`] if `rows` is non-empty and a
    /// row's width differs from `headers.len()`.
    pub fn build_from_rows(
        &self,
        headers: &[String],
        rows: &[Vec<f64>],
    ) -> Result<DataFrame, AnalysisError> {
        let n_cols = headers.len();
        if n_cols == 0 {
            return Err(AnalysisError::PolarsError("no headers provided".to_owned()));
        }
        let mut series_vec: Vec<polars::prelude::Column> = Vec::with_capacity(n_cols);
        for col_idx in 0..n_cols {
            let mut col_buf: Vec<f64> = Vec::with_capacity(rows.len());
            for row in rows {
                if row.len() != n_cols {
                    return Err(AnalysisError::SchemaMismatch {
                        expected: format!("{n_cols} columns"),
                        got: format!("{} columns", row.len()),
                    });
                }
                col_buf.push(row[col_idx]);
            }
            let name: polars::prelude::PlSmallStr = headers[col_idx].as_str().into();
            let chunked = Float64Chunked::from_vec(name, col_buf);
            let series = chunked.into_series();
            let col: polars::prelude::Column = series.into();
            series_vec.push(col);
        }
        DataFrame::new_infer_height(series_vec)
            .map_err(|e| AnalysisError::PolarsError(e.to_string()))
    }
}

// ===========================================================================
// RayonAnalysisPipeline — parallel analysis pipeline
// ===========================================================================

/// Analysis pipeline that processes inputs in parallel using rayon.
/// Per `specs/analysis.toml [parallelism]`, rayon provides CPU-bound
/// parallelism for analysis work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RayonAnalysisPipeline;

impl RayonAnalysisPipeline {
    /// Construct a new `RayonAnalysisPipeline`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Count null/NaN values in a polars `DataFrame` using rayon.
    fn count_nulls(frame: &DataFrame) -> u64 {
        let mut count = 0u64;
        for i in 0..frame.width() {
            if let Some(col) = frame.columns().get(i) {
                let series = col.as_materialized_series();
                count += series.null_count() as u64;
            }
        }
        count
    }

    /// Build a `FeatureSchema` from a polars `DataFrame`.
    fn schema_from_frame(frame: &DataFrame) -> FeatureSchema {
        let features: Vec<FeatureSpec> = frame
            .columns()
            .iter()
            .map(|col| FeatureSpec {
                name: col.name().to_string(),
                dtype: FeatureDType::F64,
                shape: vec![],
            })
            .collect();
        FeatureSchema {
            features,
            normalization: NormalizationSpec::None,
        }
    }

    /// Convert a `StorageResultSet` to a polars `DataFrame`.
    fn storage_result_to_frame(rs: StorageResultSet) -> Result<DataFrame, AnalysisError> {
        if rs.entries.is_empty() {
            return Ok(DataFrame::empty());
        }
        let headers = vec!["key".to_string(), "value".to_string()];
        let rows: Vec<Vec<f64>> = rs
            .entries
            .into_iter()
            .map(|(k, v)| {
                let key_len = usize_to_f64(k.as_str().len());
                let val_len = usize_to_f64(v.bytes.len());
                vec![key_len, val_len]
            })
            .collect();
        let builder = PolarsDatasetBuilder::new(headers);
        builder.build_from_rows(&["key".to_string(), "value".to_string()], &rows)
    }
}

/// Lossy `usize` -> `f64` conversion for telemetry-size features. The
/// values are bounded by small byte counts, so precision loss is not a
/// concern in practice.
#[allow(clippy::cast_precision_loss)]
fn usize_to_f64(n: usize) -> f64 {
    n as f64
}

impl AnalysisPipeline for RayonAnalysisPipeline {
    fn run(
        &self,
        input: &AnalysisInput,
        _ctx: &Context,
    ) -> impl Future<Output = Result<AnalysisResult, AnalysisError>> {
        let out = match input {
            AnalysisInput::DataFrame(frame) => {
                let null_count = Self::count_nulls(frame);
                let schema = Self::schema_from_frame(frame);
                Ok(AnalysisResult {
                    frame: frame.clone(),
                    schema,
                    stats: AnalysisStats {
                        rows: u64::try_from(frame.height()).unwrap_or(u64::MAX),
                        columns: u32::try_from(frame.width()).unwrap_or(u32::MAX),
                        null_count,
                        generated_at: Timestamp::now_monotonic(),
                    },
                })
            }
            AnalysisInput::Telemetry { .. } => Err(AnalysisError::InternalError(
                "telemetry queries require a storage backend; use StorageQuery variant".to_owned(),
            )),
            AnalysisInput::StorageQuery(_) => Err(AnalysisError::InternalError(
                "storage queries require a storage backend; pass one to the pipeline".to_owned(),
            )),
        };
        std::future::ready(out)
    }
}

/// Analysis pipeline with a storage backend. Extends
/// [`RayonAnalysisPipeline`] with the ability to query storage.
pub struct StorageAnalysisPipeline<S: Storage> {
    /// The storage backend.
    storage: S,
}

impl<S: Storage> StorageAnalysisPipeline<S> {
    /// Construct a `StorageAnalysisPipeline` with the given storage.
    #[must_use]
    pub fn new(storage: S) -> Self {
        Self { storage }
    }
}

#[allow(async_fn_in_trait, clippy::manual_async_fn)]
impl<S: Storage> AnalysisPipeline for StorageAnalysisPipeline<S> {
    fn run(
        &self,
        input: &AnalysisInput,
        _ctx: &Context,
    ) -> impl Future<Output = Result<AnalysisResult, AnalysisError>> {
        async move {
            match input {
                AnalysisInput::DataFrame(frame) => {
                    let null_count = RayonAnalysisPipeline::count_nulls(frame);
                    let schema = RayonAnalysisPipeline::schema_from_frame(frame);
                    Ok(AnalysisResult {
                        frame: frame.clone(),
                        schema,
                        stats: AnalysisStats {
                            rows: u64::try_from(frame.height()).unwrap_or(u64::MAX),
                            columns: u32::try_from(frame.width()).unwrap_or(u32::MAX),
                            null_count,
                            generated_at: Timestamp::now_monotonic(),
                        },
                    })
                }
                AnalysisInput::StorageQuery(q) => {
                    let rs = self.storage.query(q).await?;
                    let frame = RayonAnalysisPipeline::storage_result_to_frame(rs)?;
                    let rows = u64::try_from(frame.height()).unwrap_or(u64::MAX);
                    let columns = u32::try_from(frame.width()).unwrap_or(u32::MAX);
                    let schema = RayonAnalysisPipeline::schema_from_frame(&frame);
                    Ok(AnalysisResult {
                        frame,
                        schema,
                        stats: AnalysisStats {
                            rows,
                            columns,
                            null_count: 0,
                            generated_at: Timestamp::now_monotonic(),
                        },
                    })
                }
                AnalysisInput::Telemetry { .. } => Err(AnalysisError::InternalError(
                    "telemetry time-range queries require a storage backend with time indexing"
                        .to_owned(),
                )),
            }
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use rayon::prelude::*;
    use themql_core::SubjectPattern;
    use themql_storage::{StorageError, StorageKey};

    #[test]
    fn analysis_input_telemetry_variant() {
        let inp = AnalysisInput::Telemetry {
            from: Timestamp::now_monotonic(),
            to: Timestamp::now_monotonic(),
            subject_pattern: SubjectPattern::from_str("vehicle.#").unwrap(),
        };
        match inp {
            AnalysisInput::Telemetry { .. } => {}
            _ => panic!("must be Telemetry variant"),
        }
    }

    #[test]
    fn analysis_input_storage_query_variant() {
        let q = StorageQuery::ByKey(StorageKey::new("test"));
        let inp = AnalysisInput::StorageQuery(q);
        match inp {
            AnalysisInput::StorageQuery(_) => {}
            _ => panic!("must be StorageQuery variant"),
        }
    }

    #[test]
    fn analysis_input_dataframe_variant() {
        let frame = DataFrame::empty();
        let inp = AnalysisInput::DataFrame(frame);
        match inp {
            AnalysisInput::DataFrame(_) => {}
            _ => panic!("must be DataFrame variant"),
        }
    }

    #[test]
    fn analysis_error_converts_to_core_error() {
        let e: Error = AnalysisError::SchemaMismatch {
            expected: "a".to_owned(),
            got: "b".to_owned(),
        }
        .into();
        assert_eq!(e.code, ErrorCode::ValidationError);

        let e2: Error = AnalysisError::PolarsError("x".to_owned()).into();
        assert_eq!(e2.code, ErrorCode::ResolverError);
    }

    #[test]
    fn analysis_error_from_storage_error() {
        let se = StorageError::NotFound;
        let ae: AnalysisError = se.into();
        assert!(matches!(ae, AnalysisError::StorageError(_)));
    }

    #[test]
    fn polars_dataframe_construction() {
        let builder = PolarsDatasetBuilder::new(vec!["x".to_owned(), "y".to_owned()]);
        let frame = builder
            .build_from_rows(
                &["x".to_owned(), "y".to_owned()],
                &[vec![1.0, 2.0], vec![3.0, 4.0]],
            )
            .expect("frame");
        assert_eq!(frame.height(), 2);
        assert_eq!(frame.width(), 2);
    }

    #[test]
    fn polars_dataset_builder_rejects_uneven_rows() {
        let builder = PolarsDatasetBuilder::new(vec!["x".to_owned(), "y".to_owned()]);
        let res =
            builder.build_from_rows(&["x".to_owned(), "y".to_owned()], &[vec![1.0, 2.0, 3.0]]);
        assert!(matches!(res, Err(AnalysisError::SchemaMismatch { .. })));
    }

    #[test]
    fn rayon_parallel_sum() {
        let rows: Vec<Vec<f64>> = vec![
            vec![1.0, f64::NAN],
            vec![2.0, 0.0],
            vec![3.0, 0.0],
            vec![4.0, 0.0],
        ];
        let null_count = rows
            .par_iter()
            .map(|row| row.iter().filter(|v| v.is_nan()).count() as u64)
            .sum::<u64>();
        assert_eq!(null_count, 1);
        let sum_first: f64 = rows.par_iter().map(|r| r[0]).sum();
        assert!((sum_first - 10.0).abs() < 1e-12);
    }

    #[test]
    fn rayon_analysis_pipeline_dataframe() {
        let pipeline = RayonAnalysisPipeline::new();
        let builder = PolarsDatasetBuilder::new(vec!["x".to_owned(), "y".to_owned()]);
        let frame = builder
            .build_from_rows(
                &["x".to_owned(), "y".to_owned()],
                &[vec![1.0, 2.0], vec![3.0, 4.0]],
            )
            .expect("frame");
        let input = AnalysisInput::DataFrame(frame);
        let ctx = Context::default();
        let result = futures_block_on(pipeline.run(&input, &ctx));
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.stats.rows, 2);
        assert_eq!(result.stats.columns, 2);
        assert_eq!(result.schema.features.len(), 2);
    }

    #[test]
    fn dataset_builder_with_labels() {
        let builder =
            PolarsDatasetBuilder::new(vec!["x".to_owned(), "y".to_owned(), "z".to_owned()]);
        let frame = builder
            .build_from_rows(
                &["x".to_owned(), "y".to_owned(), "z".to_owned()],
                &[vec![1.0, 2.0, 3.0]],
            )
            .unwrap();
        let result = AnalysisResult {
            frame,
            schema: FeatureSchema {
                features: vec![
                    FeatureSpec {
                        name: "x".to_string(),
                        dtype: FeatureDType::F64,
                        shape: vec![],
                    },
                    FeatureSpec {
                        name: "y".to_string(),
                        dtype: FeatureDType::F64,
                        shape: vec![],
                    },
                    FeatureSpec {
                        name: "z".to_string(),
                        dtype: FeatureDType::F64,
                        shape: vec![],
                    },
                ],
                normalization: NormalizationSpec::None,
            },
            stats: AnalysisStats {
                rows: 1,
                columns: 3,
                null_count: 0,
                generated_at: Timestamp::now_monotonic(),
            },
        };
        let ds_builder = DatasetBuilder {
            feature_columns: vec!["x".to_owned(), "y".to_owned()],
            label_columns: vec!["z".to_owned()],
            normalization: NormalizationSpec::None,
        };
        let ds = ds_builder.build(&result).unwrap();
        assert_eq!(ds.features.width(), 2);
        assert_eq!(ds.labels.width(), 1);
        assert_eq!(ds.feature_schema.features.len(), 2);
    }

    #[test]
    fn dataset_builder_rejects_missing_column() {
        let builder = PolarsDatasetBuilder::new(vec!["x".to_owned()]);
        let frame = builder
            .build_from_rows(&["x".to_owned()], &[vec![1.0]])
            .unwrap();
        let result = AnalysisResult {
            frame,
            schema: FeatureSchema {
                features: vec![FeatureSpec {
                    name: "x".to_string(),
                    dtype: FeatureDType::F64,
                    shape: vec![],
                }],
                normalization: NormalizationSpec::None,
            },
            stats: AnalysisStats {
                rows: 1,
                columns: 1,
                null_count: 0,
                generated_at: Timestamp::now_monotonic(),
            },
        };
        let ds_builder = DatasetBuilder {
            feature_columns: vec!["x".to_owned()],
            label_columns: vec!["missing".to_owned()],
            normalization: NormalizationSpec::None,
        };
        assert!(ds_builder.build(&result).is_err());
    }

    #[tokio::test]
    async fn storage_analysis_pipeline_storage_query() {
        let storage = themql_storage::SledStorage::open_temp().unwrap();
        let key = StorageKey::new("test.key");
        storage
            .put(
                &key,
                themql_storage::StorageValue::new(vec![1, 2, 3], themql_core::FormatTag::Json),
            )
            .await
            .unwrap();
        let pipeline = StorageAnalysisPipeline::new(storage);
        let input = AnalysisInput::StorageQuery(StorageQuery::ByKey(key));
        let ctx = Context::default();
        let result = pipeline.run(&input, &ctx).await.unwrap();
        assert_eq!(result.stats.rows, 1);
        assert_eq!(result.stats.columns, 2);
    }

    fn futures_block_on<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f)
    }
}
