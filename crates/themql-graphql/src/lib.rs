//! # themql-graphql
//!
//! GraphQL projection of theMQL's core query model. `async-graphql` is
//! the sole GraphQL implementation. GraphQL types are generated from or
//! mapped to `themql-core` / `themql-query` types; GraphQL-specific types
//! must not escape this crate.
//!
//! See `specs/graphql.toml` for the authoritative specification. This
//! v0.1 crate declares the public bridge trait, the marker root types,
//! and the error enum; a real `async_graphql::Schema` build is added in
//! a later phase.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(async_fn_in_trait)]

use std::future::Future;

use themql_core::{Context, Error};
use thiserror::Error;

use async_graphql::Object;

// ===========================================================================
// Marker root types — concrete fields wired via #[Object]
// ===========================================================================

/// GraphQL `Query` root. Each field delegates to a `themql-core`
/// `Resolver` via [`GraphqlResolverBridge`].
#[derive(Debug, Clone, Copy, Default)]
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Placeholder field — returns a static string. Real fields are
    /// generated from configured resolvers in a future phase.
    async fn placeholder(&self) -> String {
        "themql-graphql query root".to_owned()
    }
}

/// GraphQL `Mutation` root. Delegates command dispatch to a
/// `themql_core::MessageHandler`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Placeholder mutation — returns a static string.
    async fn placeholder(&self) -> String {
        "themql-graphql mutation root".to_owned()
    }
}

/// GraphQL `Subscription` root marker. Maps to `themql-message` streams
/// via the SSE / MQTT bridge. Concrete subscription fields require a
/// `Stream` type and are added in a future phase.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubscriptionRoot;

// ===========================================================================
// GraphqlSchema — built async-graphql Schema, ready to serve
// ===========================================================================

/// Built `async-graphql` schema, ready to serve. Implementations hold a
/// constructed `async_graphql::Schema<QueryRoot, MutationRoot,
/// SubscriptionRoot>` and return a reference to it.
pub trait GraphqlSchema: Send + Sync {
    /// The built `async_graphql::Schema`. The roots are the marker types
    /// [`QueryRoot`], [`MutationRoot`], [`SubscriptionRoot`]; a concrete
    /// async-graphql schema build wires up real resolvers and is added
    /// in a later phase.
    fn schema(&self) -> &async_graphql::Schema<QueryRoot, MutationRoot, SubscriptionRoot>;
}

// ===========================================================================
// GraphqlResolverBridge — maps a GraphQL field to a core Resolver call
// ===========================================================================

/// Bridge that maps a GraphQL field resolution to a `themql-core`
/// `Resolver` call. Implementations are domain-specific.
pub trait GraphqlResolverBridge: Send + Sync {
    /// Resolve a GraphQL field by name with the given JSON arguments and
    /// execution context.
    ///
    /// # Errors
    /// Returns [`Error`] if the field cannot be resolved.
    fn resolve_field(
        &self,
        field: &str,
        args: &serde_json::Value,
        ctx: &Context,
    ) -> impl Future<Output = Result<serde_json::Value, Error>>;
}

// ===========================================================================
// Error
// ===========================================================================

/// Errors raised by the GraphQL adapter.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum GraphqlError {
    /// The `async_graphql::Schema` could not be built.
    #[error("graphql schema build failed: {0}")]
    SchemaBuildFailed(String),
    /// A GraphQL field resolution failed.
    #[error("graphql resolution failed: {0}")]
    ResolutionFailed(String),
    /// A GraphQL type could not be mapped to / from a core type.
    #[error("graphql mapping error: {0}")]
    MappingError(String),
}

impl From<GraphqlError> for Error {
    fn from(e: GraphqlError) -> Self {
        match e {
            GraphqlError::SchemaBuildFailed(_) | GraphqlError::MappingError(_) => {
                Error::internal_error(e.to_string())
            }
            GraphqlError::ResolutionFailed(_) => Error::resolver_error(e.to_string()),
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use themql_core::ErrorCode;

    #[test]
    fn marker_roots_are_constructible() {
        let _: QueryRoot = QueryRoot;
        let _: MutationRoot = MutationRoot;
        let _: SubscriptionRoot = SubscriptionRoot;
    }

    #[test]
    fn graphql_error_schema_build_maps_to_internal_error() {
        let e: Error = GraphqlError::SchemaBuildFailed("bad sdl".to_owned()).into();
        assert_eq!(e.code, ErrorCode::InternalError);
    }

    #[test]
    fn graphql_error_mapping_maps_to_internal_error() {
        let e: Error = GraphqlError::MappingError("no field".to_owned()).into();
        assert_eq!(e.code, ErrorCode::InternalError);
    }

    #[test]
    fn graphql_error_resolution_maps_to_resolver_error() {
        let e: Error = GraphqlError::ResolutionFailed("field x".to_owned()).into();
        assert_eq!(e.code, ErrorCode::ResolverError);
    }

    #[tokio::test]
    async fn graphql_schema_builds_with_placeholder_fields() {
        use async_graphql::EmptySubscription;
        let schema =
            async_graphql::Schema::build(QueryRoot, MutationRoot, EmptySubscription).finish();
        let q = r"{ placeholder }";
        let result = schema.execute(q).await;
        let data = result.data.into_json().expect("data to json");
        assert_eq!(
            data["placeholder"], "themql-graphql query root",
            "placeholder field must resolve"
        );
    }

    #[tokio::test]
    async fn graphql_mutation_placeholder_resolves() {
        use async_graphql::EmptySubscription;
        let schema =
            async_graphql::Schema::build(QueryRoot, MutationRoot, EmptySubscription).finish();
        let q = r"mutation { placeholder }";
        let result = schema.execute(q).await;
        let data = result.data.into_json().expect("data to json");
        assert_eq!(
            data["placeholder"], "themql-graphql mutation root",
            "mutation placeholder must resolve"
        );
    }
}
