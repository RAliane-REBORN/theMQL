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

use themql_core::{
    Context, Error, Message, MessageHandler, Query, Resource, ResponseValue, Subject,
};
use thiserror::Error;

use async_graphql::{Guard, Object, Subscription};

use futures_util::stream::Stream;

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
// GraphqlSubscriptionSource — dyn-compatible source of live event streams
// ===========================================================================

/// A boxed, pinned stream of JSON values used by subscription sources.
pub type JsonValueStream = Box<dyn Stream<Item = serde_json::Value> + Send + Unpin>;

/// A source of live event streams keyed by subject. Implementations are
/// typically backed by `themql-sse`'s `TokioSsePublisher` (broadcast
/// channel) but any transport that can produce a stream of JSON values
/// for a given subject can implement this trait.
///
/// This trait is dyn-compatible so `SubscriptionRoot` can hold
/// `Arc<dyn GraphqlSubscriptionSource>`.
pub trait GraphqlSubscriptionSource: Send + Sync {
    /// Subscribe to `subject` and return a stream of JSON values, one
    /// per published event.
    ///
    /// # Errors
    /// Returns [`Error`] if the subscription cannot be established.
    fn subscribe_stream<'a>(
        &'a self,
        subject: &'a Subject,
    ) -> Pin<Box<dyn Future<Output = Result<JsonValueStream, Error>> + Send + 'a>>;
}

/// Wrapper that adapts a `themql_sse::SseStream` into a
/// `Stream<Item = serde_json::Value>`.
///
/// Uses a background task to pull events from the `SseStream` and
/// forward them through a channel, avoiding self-referential borrow
/// issues.
struct SseStreamAdapter {
    rx: tokio::sync::mpsc::Receiver<serde_json::Value>,
}

impl SseStreamAdapter {
    fn new(mut stream: Box<dyn themql_sse::SseStream>) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
        tokio::spawn(async move {
            while let Ok(Some(event)) = stream.next_event().await {
                let value = event_to_json(&event);
                if tx.send(value).await.is_err() {
                    break;
                }
            }
        });
        Self { rx }
    }
}

fn event_to_json(event: &themql_sse::SseEvent) -> serde_json::Value {
    let data = serde_json::from_str(&event.data)
        .unwrap_or_else(|_| serde_json::Value::String(event.data.clone()));
    serde_json::json!({
        "id": event.id,
        "event": event.event,
        "data": data,
        "retry": event.retry,
    })
}

impl Stream for SseStreamAdapter {
    type Item = serde_json::Value;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

impl GraphqlSubscriptionSource for themql_sse::TokioSsePublisher {
    fn subscribe_stream<'a>(
        &'a self,
        subject: &'a Subject,
    ) -> Pin<Box<dyn Future<Output = Result<JsonValueStream, Error>> + Send + 'a>> {
        Box::pin(async move {
            use themql_sse::SsePublisher;
            let stream = SsePublisher::add_subscriber(self, subject)
                .await
                .map_err(|e| Error::internal_error(e.to_string()))?;
            let adapter = SseStreamAdapter::new(stream);
            Ok(Box::new(adapter) as JsonValueStream)
        })
    }
}

// ===========================================================================
// Authz — roles and field guards
// ===========================================================================

/// User role for authorization. Per `specs/auth.toml [authz]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthRole {
    /// Full access: all queries, mutations, subscriptions.
    Admin,
    /// Operator: all queries, mutations on vehicle.*, subscriptions.
    Operator,
    /// Observer: read-only queries on vehicle.*, subscriptions only.
    Observer,
}

impl AuthRole {
    /// Returns `true` if this role satisfies the `required` role
    /// (admin > operator > observer).
    #[must_use]
    pub fn satisfies(self, required: AuthRole) -> bool {
        if self == AuthRole::Admin {
            return true;
        }
        if matches!(
            (self, required),
            (AuthRole::Operator, AuthRole::Operator | AuthRole::Observer)
                | (AuthRole::Observer, AuthRole::Observer)
        ) {
            return true;
        }
        false
    }
}

impl std::str::FromStr for AuthRole {
    type Err = ();

    /// Parse a role from a string slice, case-insensitive. Returns
    /// `Err(())` for unknown strings. Per `specs/auth.toml [authz]`, the
    /// canonical role names are `admin`, `operator`, `observer`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "admin" => Ok(AuthRole::Admin),
            "operator" => Ok(AuthRole::Operator),
            "observer" => Ok(AuthRole::Observer),
            _ => Err(()),
        }
    }
}

/// GraphQL field guard that requires a minimum [`AuthRole`]. The role
/// is read from `async_graphql::Context::data::<AuthRole>()`.
#[derive(Debug, Clone, Copy)]
pub struct RoleGuard {
    /// Minimum role required to access the field.
    pub required: AuthRole,
}

impl RoleGuard {
    /// Create a guard requiring the given role.
    #[must_use]
    pub fn new(required: AuthRole) -> Self {
        Self { required }
    }
}

#[allow(clippy::unused_async_trait_impl)]
impl Guard for RoleGuard {
    async fn check(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<()> {
        match ctx.data::<AuthRole>() {
            Ok(role) => {
                if role.satisfies(self.required) {
                    Ok(())
                } else {
                    Err(async_graphql::Error::new(format!(
                        "insufficient role: requires {:?}, have {:?}",
                        self.required, role
                    )))
                }
            }
            Err(_) => Err(async_graphql::Error::new(
                "no auth role in context (auth not enabled?)",
            )),
        }
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
    /// Requires `Observer` role (read-only access).
    #[graphql(guard = "RoleGuard::new(AuthRole::Observer)")]
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
    /// Requires `Observer` role (read-only access).
    #[graphql(guard = "RoleGuard::new(AuthRole::Observer)")]
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
    /// Requires `Operator` role (write access).
    #[graphql(guard = "RoleGuard::new(AuthRole::Operator)")]
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

/// GraphQL `Subscription` root. Maps to `themql-message` streams via
/// the SSE bridge.
///
/// When constructed with a [`GraphqlSubscriptionSource`] (typically a
/// `themql_sse::TokioSsePublisher`), `subscribe(subject)` returns a
/// real live stream of events. When constructed via
/// [`SubscriptionRoot::default`], the subscription returns an error.
#[derive(Clone, Default)]
pub struct SubscriptionRoot {
    source: Option<Arc<dyn GraphqlSubscriptionSource>>,
}

impl SubscriptionRoot {
    /// Construct a `SubscriptionRoot` backed by a live event source
    /// (e.g. `themql_sse::TokioSsePublisher`).
    #[must_use]
    pub fn with_source(source: Arc<dyn GraphqlSubscriptionSource>) -> Self {
        Self {
            source: Some(source),
        }
    }
}

#[Subscription]
impl SubscriptionRoot {
    /// Subscribe to updates for `subject`. Returns a live stream of
    /// JSON-encoded events from the underlying subscription source.
    /// Requires `Observer` role (read-only access).
    ///
    /// # Errors
    /// Returns an `async_graphql::Error` if no subscription source is
    /// configured, if the subject is invalid, or if the subscription
    /// cannot be established.
    #[graphql(guard = "RoleGuard::new(AuthRole::Observer)")]
    async fn subscribe(
        &self,
        subject: String,
    ) -> Result<impl Stream<Item = async_graphql::Json<serde_json::Value>>, async_graphql::Error>
    {
        use futures_util::StreamExt;
        let source = self
            .source
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("no subscription source configured"))?;
        let subj =
            Subject::from_str(&subject).map_err(|e| async_graphql::Error::new(e.to_string()))?;
        let stream = source
            .subscribe_stream(&subj)
            .await
            .map_err(|e| async_graphql::Error::new(e.message))?;
        Ok(stream.map(async_graphql::Json))
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
    /// roots. No auth role is injected — all guarded fields will
    /// reject requests. Use [`GraphqlSchemaImpl::with_role`] for
    /// authenticated schemas.
    #[must_use]
    pub fn new(query: QueryRoot, mutation: MutationRoot, subscription: SubscriptionRoot) -> Self {
        let schema = async_graphql::Schema::build(query, mutation, subscription).finish();
        Self { schema }
    }

    /// Build a schema with a default [`AuthRole`] injected as global
    /// data. All guarded fields will use this role for authorization.
    ///
    /// **Test-only.** Production code should use [`GraphqlSchemaImpl::new`]
    /// and inject the role per-request via [`async_graphql::Request::data`]
    /// or [`async_graphql::BatchRequest::data`], so the caller's actual
    /// session role is enforced. This constructor is retained for tests
    /// and the dev path where no auth is configured.
    #[must_use]
    pub fn with_role(
        query: QueryRoot,
        mutation: MutationRoot,
        subscription: SubscriptionRoot,
        role: AuthRole,
    ) -> Self {
        let schema = async_graphql::Schema::build(query, mutation, subscription)
            .data(role)
            .finish();
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

    #[allow(clippy::unused_async_trait_impl)]
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
            SubscriptionRoot::default(),
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
        let schema = GraphqlSchemaImpl::with_role(
            QueryRoot::new(bridge),
            MutationRoot::new(dispatch),
            SubscriptionRoot::default(),
            AuthRole::Observer,
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
        let schema = GraphqlSchemaImpl::with_role(
            QueryRoot::new(bridge),
            MutationRoot::new(dispatch),
            SubscriptionRoot::default(),
            AuthRole::Operator,
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

    #[tokio::test]
    async fn subscription_with_source_streams_real_events() {
        use futures_util::StreamExt;
        use themql_core::Operation;
        use themql_sse::{SsePublisher, TokioSsePublisher};

        let publisher = Arc::new(TokioSsePublisher::new());
        let source: Arc<dyn GraphqlSubscriptionSource> =
            Arc::clone(&publisher) as Arc<dyn GraphqlSubscriptionSource>;
        let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
        let bridge: Arc<dyn GraphqlResolverBridge> =
            Arc::new(GraphqlResolverBridgeImpl::new(resolver));
        let dispatch: Arc<dyn DispatchBridge> =
            Arc::new(DispatchBridgeImpl::new(Arc::new(StubHandler)));
        let schema = GraphqlSchemaImpl::with_role(
            QueryRoot::new(bridge),
            MutationRoot::new(dispatch),
            SubscriptionRoot::with_source(source),
            AuthRole::Observer,
        );

        let publisher_clone = Arc::clone(&publisher);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let subject = Subject::from_str("vehicle.sensors.imu").expect("subject");
            let subject_str = subject.as_str();
            let msg = Message::new(&subject_str, Operation::Event).expect("message");
            publisher_clone
                .broadcast(&subject, &msg)
                .await
                .expect("broadcast");
        });

        let q = r#"subscription { subscribe(subject: "vehicle.sensors.imu") }"#;
        let mut stream = schema.schema().execute_stream(q);
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next()).await;

        assert!(
            result.is_ok(),
            "subscription should produce at least one event"
        );
        let response = result.expect("timeout").expect("event");
        assert!(
            response.errors.is_empty(),
            "no errors expected, got {response:?}"
        );
    }

    #[tokio::test]
    async fn subscription_without_source_returns_error() {
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
        let q = r#"subscription { subscribe(subject: "vehicle.sensors.imu") }"#;
        let result = schema.schema().execute(q).await;
        assert!(
            !result.errors.is_empty(),
            "expected error for no source, got {result:?}"
        );
    }

    // --- Authz tests -------------------------------------------------------

    #[test]
    fn auth_role_satisfies_hierarchy() {
        assert!(AuthRole::Admin.satisfies(AuthRole::Admin));
        assert!(AuthRole::Admin.satisfies(AuthRole::Operator));
        assert!(AuthRole::Admin.satisfies(AuthRole::Observer));
        assert!(AuthRole::Operator.satisfies(AuthRole::Operator));
        assert!(AuthRole::Operator.satisfies(AuthRole::Observer));
        assert!(!AuthRole::Operator.satisfies(AuthRole::Admin));
        assert!(AuthRole::Observer.satisfies(AuthRole::Observer));
        assert!(!AuthRole::Observer.satisfies(AuthRole::Operator));
        assert!(!AuthRole::Observer.satisfies(AuthRole::Admin));
    }

    #[test]
    fn auth_role_from_str_maps_known_roles_case_insensitive() {
        use std::str::FromStr;
        assert_eq!(AuthRole::from_str("admin"), Ok(AuthRole::Admin));
        assert_eq!(AuthRole::from_str("Operator"), Ok(AuthRole::Operator));
        assert_eq!(AuthRole::from_str("OBSERVER"), Ok(AuthRole::Observer));
    }

    #[test]
    fn auth_role_from_str_unknown_returns_err() {
        use std::str::FromStr;
        assert!(AuthRole::from_str("root").is_err());
        assert!(AuthRole::from_str("").is_err());
        assert!(AuthRole::from_str("superuser").is_err());
    }

    #[tokio::test]
    async fn query_rejected_without_role() {
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
            !result.errors.is_empty(),
            "expected authz error, got {result:?}"
        );
        assert!(result.errors[0].message.contains("no auth role"));
    }

    #[tokio::test]
    async fn mutation_rejected_for_observer() {
        let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
        let bridge: Arc<dyn GraphqlResolverBridge> =
            Arc::new(GraphqlResolverBridgeImpl::new(resolver));
        let dispatch: Arc<dyn DispatchBridge> =
            Arc::new(DispatchBridgeImpl::new(Arc::new(StubHandler)));
        let schema = GraphqlSchemaImpl::with_role(
            QueryRoot::new(bridge),
            MutationRoot::new(dispatch),
            SubscriptionRoot::default(),
            AuthRole::Observer,
        );
        let q = r#"mutation { dispatch(subject: "vehicle.command", payload: {x: 1}) }"#;
        let result = schema.schema().execute(q).await;
        assert!(
            !result.errors.is_empty(),
            "expected authz error for observer on mutation, got {result:?}"
        );
        assert!(result.errors[0].message.contains("insufficient role"));
    }

    #[tokio::test]
    async fn mutation_allowed_for_admin() {
        let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
        let bridge: Arc<dyn GraphqlResolverBridge> =
            Arc::new(GraphqlResolverBridgeImpl::new(resolver));
        let dispatch: Arc<dyn DispatchBridge> =
            Arc::new(DispatchBridgeImpl::new(Arc::new(StubHandler)));
        let schema = GraphqlSchemaImpl::with_role(
            QueryRoot::new(bridge),
            MutationRoot::new(dispatch),
            SubscriptionRoot::default(),
            AuthRole::Admin,
        );
        let q = r#"mutation { dispatch(subject: "vehicle.command", payload: {x: 1}) }"#;
        let result = schema.schema().execute(q).await;
        assert!(
            result.errors.is_empty(),
            "expected no authz error for admin, got {result:?}"
        );
    }

    #[tokio::test]
    async fn query_allowed_for_operator() {
        let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
        let bridge: Arc<dyn GraphqlResolverBridge> =
            Arc::new(GraphqlResolverBridgeImpl::new(resolver));
        let dispatch: Arc<dyn DispatchBridge> =
            Arc::new(DispatchBridgeImpl::new(Arc::new(StubHandler)));
        let schema = GraphqlSchemaImpl::with_role(
            QueryRoot::new(bridge),
            MutationRoot::new(dispatch),
            SubscriptionRoot::default(),
            AuthRole::Operator,
        );
        let q = r#"{ resource(subject: "vehicle.sensors.imu") }"#;
        let result = schema.schema().execute(q).await;
        assert!(
            result.errors.is_empty(),
            "expected no authz error for operator on query, got {result:?}"
        );
    }
}
