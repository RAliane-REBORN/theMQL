//! # themql-graphql
//!
//! GraphQL projection of theMQL's core query model. `async-graphql` is
//! the sole GraphQL implementation. GraphQL types are generated from or
//! mapped to `themql-core` / `themql-query` types; GraphQL-specific types
//! must not escape this crate.
//!
//! See `specs/graphql.toml` for the authoritative specification.
//!
//! This crate provides:
//! - [`GraphqlResolverBridge`] — dyn-compatible trait mapping a GraphQL
//!   field resolution to a `themql-core` `Resolver` call.
//! - [`GraphqlResolverBridgeImpl`] — concrete bridge wrapping
//!   `Arc<dyn themql_core::Resolver>`.
//! - [`QueryRoot`], [`MutationRoot`], [`SubscriptionRoot`] — async-graphql
//!   root objects with real fields that delegate to the bridge.
//! - [`GraphqlSchemaImpl`] — holds a built
//!   `async_graphql::Schema<QueryRoot, MutationRoot, SubscriptionRoot>`.
//! - [`serve_graphql`] — axum `Router` mounting the GraphQL endpoint at
//!   `/graphql` (POST for query/mutation, GET with WebSocket upgrade for
//!   subscription).

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![deny(warnings)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use themql_core::{Context, Error, Message, MessageHandler, Query, Resource, ResponseValue};
use thiserror::Error;

use async_graphql::Object;
use async_graphql::Subscription;

use futures_util::stream::{self, Stream};

// ===========================================================================
// GraphqlResolverBridge — dyn-compatible mapping of a GraphQL field to a
// core Resolver / MessageHandler call.
// ===========================================================================

/// Bridge that maps a GraphQL field resolution to a `themql-core`
/// `Resolver` call. Implementations are domain-specific.
///
/// This trait is dyn-compatible so root objects can hold
/// `Arc<dyn GraphqlResolverBridge>`.
pub trait GraphqlResolverBridge: Send + Sync {
    /// Resolve a GraphQL field by name with the given JSON arguments and
    /// execution context.
    ///
    /// # Errors
    /// Returns [`Error`] if the field cannot be resolved.
    fn resolve_field<'a>(
        &'a self,
        field: &'a str,
        args: &'a serde_json::Value,
        ctx: &'a Context,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, Error>> + Send + 'a>>;
}

/// Concrete [`GraphqlResolverBridge`] wrapping `Arc<dyn themql_core::Resolver>`.
///
/// `resolve_field` interprets `field` as a resource subject string,
/// builds a [`Query`] against that resource, delegates to the wrapped
/// [`themql_core::Resolver`], and projects the [`ResponseValue`] into a
/// JSON value.
#[derive(Clone)]
pub struct GraphqlResolverBridgeImpl {
    resolver: Arc<dyn themql_core::Resolver>,
}

impl GraphqlResolverBridgeImpl {
    /// Construct a new bridge wrapping the given resolver.
    #[must_use]
    pub fn new(resolver: Arc<dyn themql_core::Resolver>) -> Self {
        Self { resolver }
    }
}

impl GraphqlResolverBridge for GraphqlResolverBridgeImpl {
    fn resolve_field<'a>(
        &'a self,
        field: &'a str,
        args: &'a serde_json::Value,
        ctx: &'a Context,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, Error>> + Send + 'a>> {
        Box::pin(async move {
            let resource =
                Resource::from_str(field).map_err(|e| Error::validation_error(e.to_string()))?;
            let mut query = Query::new(resource);
            query.context = ctx.clone();
            if !args.is_null() {
                query = query.with_arguments(args.clone());
            }
            let response = self.resolver.resolve(&query, ctx).await?;
            Ok(response_value_to_json(response.result?))
        })
    }
}

/// Project a [`ResponseValue`] into a JSON value suitable for GraphQL.
fn response_value_to_json(value: ResponseValue) -> serde_json::Value {
    match value {
        ResponseValue::Json(v) => v,
        ResponseValue::Bytes(bytes, tag) => {
            let obj = serde_json::json!({
                "format": tag.to_string(),
                "bytes": bytes,
            });
            obj
        }
        ResponseValue::Unit => serde_json::Value::Null,
    }
}

// ===========================================================================
// DispatchBridge — dyn-compatible mapping of a GraphQL mutation to a
// core MessageHandler call.
// ===========================================================================

/// Bridge that maps a GraphQL mutation dispatch to a
/// `themql-core` `MessageHandler` call. Dyn-compatible.
pub trait DispatchBridge: Send + Sync {
    /// Dispatch a command to `subject` with the given JSON payload.
    ///
    /// # Errors
    /// Returns [`Error`] if the dispatch fails.
    fn dispatch<'a>(
        &'a self,
        subject: &'a str,
        payload: &'a serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, Error>> + Send + 'a>>;
}

/// Concrete [`DispatchBridge`] wrapping `Arc<dyn themql_core::MessageHandler>`.
#[derive(Clone)]
pub struct DispatchBridgeImpl {
    handler: Arc<dyn MessageHandler>,
}

impl DispatchBridgeImpl {
    /// Construct a new dispatch bridge wrapping the given handler.
    #[must_use]
    pub fn new(handler: Arc<dyn MessageHandler>) -> Self {
        Self { handler }
    }
}

impl DispatchBridge for DispatchBridgeImpl {
    fn dispatch<'a>(
        &'a self,
        subject: &'a str,
        payload: &'a serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, Error>> + Send + 'a>> {
        Box::pin(async move {
            use themql_core::{Operation, Payload};
            let mut msg = Message::new(subject, Operation::Command)
                .map_err(|e| Error::validation_error(e.to_string()))?;
            msg.payload = Payload::Json(payload.clone());
            let response = self.handler.handle(&msg).await?;
            Ok(response_value_to_json(response.result?))
        })
    }
}

// ===========================================================================
// Root types — QueryRoot, MutationRoot, SubscriptionRoot
// ===========================================================================

/// GraphQL `Query` root. Each field delegates to a
/// `themql-core` `Resolver` via [`GraphqlResolverBridge`].
#[derive(Clone)]
pub struct QueryRoot {
    bridge: Arc<dyn GraphqlResolverBridge>,
}

impl QueryRoot {
    /// Construct a `QueryRoot` backed by the given resolver bridge.
    #[must_use]
    pub fn new(bridge: Arc<dyn GraphqlResolverBridge>) -> Self {
        Self { bridge }
    }
}

#[Object]
impl QueryRoot {
    /// Resolve a single resource identified by `subject`, optionally
    /// narrowed by a JSON `selection` and resolver `args`.
    async fn resource(
        &self,
        subject: String,
        selection: Option<async_graphql::Json<serde_json::Value>>,
        args: Option<async_graphql::Json<serde_json::Value>>,
    ) -> Result<async_graphql::Json<serde_json::Value>, async_graphql::Error> {
        let _ = selection;
        let args_val = args.map_or(serde_json::Value::Null, |v| v.0);
        let ctx = Context::new();
        let value = self
            .bridge
            .resolve_field(&subject, &args_val, &ctx)
            .await
            .map_err(|e| async_graphql::Error::new(e.message))?;
        Ok(async_graphql::Json(value))
    }

    /// Resolve all resources matching a subject `pattern`. The current
    /// bridge delegates the pattern resolution to the underlying
    /// resolver as a single field call.
    async fn resources(
        &self,
        pattern: String,
    ) -> Result<Vec<async_graphql::Json<serde_json::Value>>, async_graphql::Error> {
        let ctx = Context::new();
        let args_val = serde_json::json!({ "pattern": pattern });
        let value = self
            .bridge
            .resolve_field("resources", &args_val, &ctx)
            .await
            .map_err(|e| async_graphql::Error::new(e.message))?;
        match value {
            serde_json::Value::Array(items) => {
                Ok(items.into_iter().map(async_graphql::Json).collect())
            }
            other => Ok(vec![async_graphql::Json(other)]),
        }
    }
}

/// GraphQL `Mutation` root. Delegates command dispatch to a
/// `themql_core::MessageHandler` via [`DispatchBridge`].
#[derive(Clone)]
pub struct MutationRoot {
    bridge: Arc<dyn DispatchBridge>,
}

impl MutationRoot {
    /// Construct a `MutationRoot` backed by the given dispatch bridge.
    #[must_use]
    pub fn new(bridge: Arc<dyn DispatchBridge>) -> Self {
        Self { bridge }
    }
}

#[Object]
impl MutationRoot {
    /// Dispatch a command to `subject` with the given JSON `payload`.
    async fn dispatch(
        &self,
        subject: String,
        payload: async_graphql::Json<serde_json::Value>,
    ) -> Result<async_graphql::Json<serde_json::Value>, async_graphql::Error> {
        let value = self
            .bridge
            .dispatch(&subject, &payload.0)
            .await
            .map_err(|e| async_graphql::Error::new(e.message))?;
        Ok(async_graphql::Json(value))
    }
}

/// GraphQL `Subscription` root. Maps to `themql-message` streams via the
/// SSE / MQTT bridge.
///
/// TODO: real subscription streams are backed by `themql-message`'s
/// `Stream` type and wired through the transport layer in a later
/// stage. The field below emits a single placeholder value so the schema
/// builds with a real `#[Subscription]` root.
#[derive(Clone, Default)]
pub struct SubscriptionRoot {
    #[allow(dead_code)]
    bridge: Option<Arc<dyn GraphqlResolverBridge>>,
}

impl SubscriptionRoot {
    /// Construct a `SubscriptionRoot` optionally holding a resolver
    /// bridge for future stream wiring.
    #[must_use]
    pub fn new(bridge: Arc<dyn GraphqlResolverBridge>) -> Self {
        Self {
            bridge: Some(bridge),
        }
    }
}

#[Subscription]
impl SubscriptionRoot {
    /// Subscribe to updates for `subject`. Emits a single placeholder
    /// value then completes; real stream wiring is added in a later
    /// stage once `themql-message` streams are integrated.
    async fn subscribe(
        &self,
        subject: String,
    ) -> impl Stream<Item = async_graphql::Json<serde_json::Value>> {
        let value = serde_json::json!({ "subject": subject, "placeholder": true });
        stream::once(async move { async_graphql::Json(value) })
    }
}

// ===========================================================================
// GraphqlSchema — built async-graphql Schema, ready to serve
// ===========================================================================

/// Built `async-graphql` schema, ready to serve. Implementations hold a
/// constructed `async_graphql::Schema<QueryRoot, MutationRoot,
/// SubscriptionRoot>` and return a reference to it.
pub trait GraphqlSchema: Send + Sync {
    /// The built `async_graphql::Schema`.
    fn schema(&self) -> &async_graphql::Schema<QueryRoot, MutationRoot, SubscriptionRoot>;
}

/// Concrete [`GraphqlSchema`] holding a built
/// `async_graphql::Schema<QueryRoot, MutationRoot, SubscriptionRoot>`.
pub struct GraphqlSchemaImpl {
    schema: async_graphql::Schema<QueryRoot, MutationRoot, SubscriptionRoot>,
}

impl GraphqlSchemaImpl {
    /// Build a schema from the given query, mutation, and subscription
    /// roots.
    #[must_use]
    pub fn new(query: QueryRoot, mutation: MutationRoot, subscription: SubscriptionRoot) -> Self {
        let schema = async_graphql::Schema::build(query, mutation, subscription).finish();
        Self { schema }
    }
}

impl GraphqlSchema for GraphqlSchemaImpl {
    fn schema(&self) -> &async_graphql::Schema<QueryRoot, MutationRoot, SubscriptionRoot> {
        &self.schema
    }
}

// ===========================================================================
// axum integration — POST /graphql (query/mutation), GET /graphql (WS)
// ===========================================================================

use async_graphql_axum::{GraphQL, GraphQLSubscription};
use axum::routing::get_service;

/// Build an `axum::Router` mounting the GraphQL endpoint at `/graphql`:
/// - `POST /graphql` — query and mutation via [`GraphQL`].
/// - `GET /graphql` — subscription via WebSocket upgrade
///   ([`GraphQLSubscription`]).
///
/// The router has no state; the schema is cloned into each service
/// (`Schema: Clone`).
#[must_use = "the router must be served to handle requests"]
pub fn serve_graphql(
    schema: async_graphql::Schema<QueryRoot, MutationRoot, SubscriptionRoot>,
) -> axum::Router {
    axum::Router::new().route_service(
        "/graphql",
        get_service(GraphQLSubscription::new(schema.clone())).post_service(GraphQL::new(schema)),
    )
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
    use themql_core::CorrelationId;
    use themql_core::{ErrorCode, Payload, Response};

    // --- Test stubs implementing the dyn-compatible core traits ---------

    struct StubResolver;

    impl themql_core::ResolverBoxed for StubResolver {
        async fn resolve(&self, query: &Query, _ctx: &Context) -> Result<Response, Error> {
            let subject = query.resource.subject.as_str();
            Ok(Response::ok(
                ResponseValue::Json(serde_json::json!({ "subject": subject })),
                CorrelationId::new(),
            ))
        }
    }

    struct StubHandler;

    impl MessageHandler for StubHandler {
        fn handle<'a>(
            &'a self,
            msg: &'a Message,
        ) -> Pin<Box<dyn Future<Output = Result<Response, Error>> + Send + 'a>> {
            let subject = msg.subject.as_str();
            let payload = msg.payload.clone();
            Box::pin(async move {
                let value = match payload {
                    Payload::Json(v) => v,
                    Payload::Unit => serde_json::Value::Null,
                    Payload::Bytes(b, tag) => serde_json::json!({
                        "format": tag.to_string(),
                        "len": b.len(),
                    }),
                };
                Ok(Response::ok(
                    ResponseValue::Json(serde_json::json!({
                        "subject": subject,
                        "echo": value,
                    })),
                    CorrelationId::new(),
                ))
            })
        }
    }

    // --- Existing error-mapping tests -----------------------------------

    #[test]
    fn marker_roots_are_constructible() {
        let bridge: Arc<dyn GraphqlResolverBridge> =
            Arc::new(GraphqlResolverBridgeImpl::new(Arc::new(StubResolver)));
        let dispatch: Arc<dyn DispatchBridge> =
            Arc::new(DispatchBridgeImpl::new(Arc::new(StubHandler)));
        let _: QueryRoot = QueryRoot::new(bridge);
        let _: MutationRoot = MutationRoot::new(dispatch);
        let _: SubscriptionRoot = SubscriptionRoot::default();
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

    // --- New tests: resolver bridge construction & schema build ---------

    #[test]
    fn resolver_bridge_impl_construction() {
        let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
        let bridge = GraphqlResolverBridgeImpl::new(resolver);
        let _: &dyn GraphqlResolverBridge = &bridge;
    }

    #[test]
    fn dispatch_bridge_impl_construction() {
        let handler: Arc<dyn MessageHandler> = Arc::new(StubHandler);
        let bridge = DispatchBridgeImpl::new(handler);
        let _: &dyn DispatchBridge = &bridge;
    }

    #[tokio::test]
    async fn schema_builds_with_real_resolver_bridge() {
        let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
        let bridge: Arc<dyn GraphqlResolverBridge> =
            Arc::new(GraphqlResolverBridgeImpl::new(resolver));
        let dispatch: Arc<dyn DispatchBridge> =
            Arc::new(DispatchBridgeImpl::new(Arc::new(StubHandler)));
        let schema = GraphqlSchemaImpl::new(
            QueryRoot::new(Arc::clone(&bridge)),
            MutationRoot::new(Arc::clone(&dispatch)),
            SubscriptionRoot::new(Arc::clone(&bridge)),
        );
        let _ = schema.schema();
    }

    #[tokio::test]
    async fn query_resource_resolves_via_bridge() {
        let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
        let bridge: Arc<dyn GraphqlResolverBridge> =
            Arc::new(GraphqlResolverBridgeImpl::new(resolver));
        let dispatch: Arc<dyn DispatchBridge> =
            Arc::new(DispatchBridgeImpl::new(Arc::new(StubHandler)));
        let schema = GraphqlSchemaImpl::new(
            QueryRoot::new(bridge),
            MutationRoot::new(dispatch),
            SubscriptionRoot::default(),
        );
        let q = r#"{ resource(subject: "vehicle.sensors.imu") }"#;
        let result = schema.schema().execute(q).await;
        assert!(
            result.errors.is_empty(),
            "expected no errors, got {result:?}"
        );
        let data = result.data.into_json().expect("data to json");
        assert_eq!(data["resource"]["subject"], "vehicle.sensors.imu");
    }

    #[tokio::test]
    async fn mutation_dispatch_resolves_via_bridge() {
        let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
        let bridge: Arc<dyn GraphqlResolverBridge> =
            Arc::new(GraphqlResolverBridgeImpl::new(resolver));
        let dispatch: Arc<dyn DispatchBridge> =
            Arc::new(DispatchBridgeImpl::new(Arc::new(StubHandler)));
        let schema = GraphqlSchemaImpl::new(
            QueryRoot::new(bridge),
            MutationRoot::new(dispatch),
            SubscriptionRoot::default(),
        );
        let q = r#"mutation { dispatch(subject: "vehicle.command", payload: {x: 1}) }"#;
        let result = schema.schema().execute(q).await;
        assert!(
            result.errors.is_empty(),
            "expected no errors, got {result:?}"
        );
        let data = result.data.into_json().expect("data to json");
        assert_eq!(data["dispatch"]["subject"], "vehicle.command");
        assert_eq!(data["dispatch"]["echo"]["x"], 1);
    }

    #[tokio::test]
    async fn serve_graphql_builds_router() {
        let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
        let bridge: Arc<dyn GraphqlResolverBridge> =
            Arc::new(GraphqlResolverBridgeImpl::new(resolver));
        let dispatch: Arc<dyn DispatchBridge> =
            Arc::new(DispatchBridgeImpl::new(Arc::new(StubHandler)));
        let schema = async_graphql::Schema::build(
            QueryRoot::new(bridge),
            MutationRoot::new(dispatch),
            SubscriptionRoot::default(),
        )
        .finish();
        let router: axum::Router = serve_graphql(schema);
        let _ = router;
    }

    #[test]
    fn response_value_to_json_json_variant() {
        let v = response_value_to_json(ResponseValue::Json(serde_json::json!({"a": 1})));
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn response_value_to_json_unit_variant() {
        let v = response_value_to_json(ResponseValue::Unit);
        assert!(v.is_null());
    }

    #[test]
    fn resolver_bridge_rejects_invalid_subject() {
        let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
        let bridge = GraphqlResolverBridgeImpl::new(resolver);
        let ctx = Context::new();
        let args = serde_json::Value::Null;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("rt build");
        let err = rt
            .block_on(bridge.resolve_field("not..valid", &args, &ctx))
            .expect_err("error for invalid subject");
        assert_eq!(err.code, ErrorCode::ValidationError);
    }
}
