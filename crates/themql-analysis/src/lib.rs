//! # themql-analysis
//!
//! Desktop analytical processing of telemetry data. Builds `Dataset`s
//! for `themql-training`, runs statistical analysis, and provides a thin
//! theDAF adapter for legacy data access and migration reference.
//!
//! Per `specs/analysis.toml`, polars is the semantic owner of dataframe
//! operations and rayon provides parallelism. Both are deferred to a
//! future task; this module defines the trait surface, minimal types,
//! and error mapping only.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::future::Future;

use serde::{Deserialize, Serialize};

use themql_core::{Context, Error, ErrorCode, SubjectPattern, Timestamp};

// ===========================================================================
// AnalysisInput
// ===========================================================================

/// Input to an [`AnalysisPipeline`]. Per `specs/analysis.toml`, the
/// canonical variants include a polars `DataFrame` and a
/// `StorageQuery`; this minimal stand-in uses `Vec<Vec<f64>>` for the
/// frame variant and omits the storage variant (the storage crate does
/// not yet export a `StorageQuery` type).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// An in-memory rectangular frame: one row per record, one column
    /// per feature.
    DataFrame(Vec<Vec<f64>>),
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

/// Result of an analysis pipeline. Per `specs/analysis.toml`, the
/// canonical shape is a polars `DataFrame` + a `FeatureSchema`; this
/// minimal stand-in holds the frame as `Vec<Vec<f64>>` and omits the
/// schema (re-exported from `themql-training` when polars is wired in).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// The result frame.
    pub frame: Vec<Vec<f64>>,
    /// Summary statistics.
    pub stats: AnalysisStats,
}

// ===========================================================================
// Dataset (minimal local stand-in; same shape as themql-training::Dataset)
// ===========================================================================

/// Minimal dataset for `themql-training`. Defined locally because
/// `themql-training::Dataset` holds polars frames in its canonical form;
/// this stand-in uses `Vec<Vec<f64>>` until polars is wired in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    /// Feature matrix.
    pub features: Vec<Vec<f64>>,
    /// Label matrix.
    pub labels: Vec<Vec<f64>>,
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
}

impl DatasetBuilder {
    /// Build a [`Dataset`] from an [`AnalysisResult`].
    ///
    /// # Errors
    /// Returns [`AnalysisError::SchemaMismatch`] if the builder's
    /// column selections cannot be satisfied by `result`.
    pub fn build(&self, result: AnalysisResult) -> Result<Dataset, AnalysisError> {
        let stats = result.stats;
        let feature_cols = u32::try_from(self.feature_columns.len()).unwrap_or(u32::MAX);
        if feature_cols > stats.columns {
            return Err(AnalysisError::SchemaMismatch {
                expected: format!("<= {} feature columns", stats.columns),
                got: format!("{} feature columns", self.feature_columns.len()),
            });
        }
        Ok(Dataset {
            features: result.frame,
            labels: Vec::new(),
        })
    }
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
    fn fetch_legacy(
        &self,
        query: &str,
    ) -> impl Future<Output = Result<Vec<Vec<f64>>, AnalysisError>>;

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
/// adapters. Mirrors `specs/analysis.toml [api.AnalysisError]`; the
/// `StorageError` variant carries a `String` rather than a
/// `themql-storage::StorageError` because that crate does not yet
/// export that type.
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
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_input_telemetry_variant() {
        let inp = AnalysisInput::Telemetry {
            from: Timestamp::now_monotonic(),
            to: Timestamp::now_monotonic(),
            subject_pattern: SubjectPattern::from_str("vehicle.#").unwrap(),
        };
        match inp {
            AnalysisInput::Telemetry { .. } => {}
            AnalysisInput::DataFrame(_) => panic!("must be Telemetry variant"),
        }
    }

    #[test]
    fn analysis_input_dataframe_variant() {
        let inp = AnalysisInput::DataFrame(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        match inp {
            AnalysisInput::DataFrame(rows) => assert_eq!(rows.len(), 2),
            AnalysisInput::Telemetry { .. } => panic!("must be DataFrame variant"),
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
    fn dataset_builder_construction() {
        let builder = DatasetBuilder {
            feature_columns: vec!["x".to_owned(), "y".to_owned()],
            label_columns: vec!["z".to_owned()],
        };
        let result = AnalysisResult {
            frame: vec![vec![1.0, 2.0], vec![3.0, 4.0]],
            stats: AnalysisStats {
                rows: 2,
                columns: 2,
                null_count: 0,
                generated_at: Timestamp::now_monotonic(),
            },
        };
        let ds = builder.build(result).unwrap();
        assert_eq!(ds.features.len(), 2);
        assert!(ds.labels.is_empty());
    }

    #[test]
    fn dataset_builder_rejects_too_many_features() {
        let builder = DatasetBuilder {
            feature_columns: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
            label_columns: vec![],
        };
        let result = AnalysisResult {
            frame: vec![vec![1.0, 2.0]],
            stats: AnalysisStats {
                rows: 1,
                columns: 2,
                null_count: 0,
                generated_at: Timestamp::now_monotonic(),
            },
        };
        assert!(matches!(
            builder.build(result),
            Err(AnalysisError::SchemaMismatch { .. })
        ));
    }
}
