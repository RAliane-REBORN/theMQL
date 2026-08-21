//! Integration test: desktop serve subcommand builds a real axum router.
//!
//! Exercises the same `GraphqlSchemaImpl::new` + `serve_graphql` +
//! `serve_sse_with_publisher` path that `serve` uses, without binding
//! a TCP listener.
//!
//! Covers `SPEC.toml [quality] integration_tests_required = true`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use themql_core::Subject;
use themql_core::{
    Context, CorrelationId, Error, Message, MessageHandler, Query, ResolverBoxed, Response,
    ResponseValue,
};
use themql_graphql::{
    DispatchBridgeImpl, GraphqlResolverBridgeImpl, GraphqlSchema, GraphqlSchemaImpl,
    GraphqlSubscriptionSource, MutationRoot, QueryRoot, SubscriptionRoot,
};
use themql_sse::{serve_sse_with_publisher, TokioSsePublisher};

use std::future::Future;
use std::pin::Pin;

struct DesktopResolver;

#[allow(clippy::unused_async_trait_impl)]
impl ResolverBoxed for DesktopResolver {
    async fn resolve(&self, query: &Query, _ctx: &Context) -> Result<Response, Error> {
        let subject = query.resource.subject.as_str();
        Ok(Response::ok(
            ResponseValue::Json(serde_json::json!({"subject": subject})),
            CorrelationId::new(),
        ))
    }
}

struct DesktopHandler;

impl MessageHandler for DesktopHandler {
    fn handle<'a>(
        &'a self,
        msg: &'a Message,
    ) -> Pin<Box<dyn Future<Output = Result<Response, Error>> + Send + 'a>> {
        let subject = msg.subject.as_str();
        Box::pin(async move {
            Ok(Response::ok(
                ResponseValue::Json(serde_json::json!({"subject": subject})),
                CorrelationId::new(),
            ))
        })
    }
}

#[tokio::test]
async fn serve_builds_real_axum_router_without_auth() {
    let resolver: Arc<dyn themql_core::Resolver> = Arc::new(DesktopResolver);
    let bridge = Arc::new(GraphqlResolverBridgeImpl::new(resolver));
    let handler: Arc<dyn themql_core::MessageHandler> = Arc::new(DesktopHandler);
    let dispatch = Arc::new(DispatchBridgeImpl::new(handler));

    let publisher = Arc::new(TokioSsePublisher::new());
    let source: Arc<dyn GraphqlSubscriptionSource> =
        Arc::clone(&publisher) as Arc<dyn GraphqlSubscriptionSource>;

    let schema = GraphqlSchemaImpl::new(
        QueryRoot::new(bridge),
        MutationRoot::new(dispatch),
        SubscriptionRoot::with_source(source),
    );
    let graphql_router = themql_graphql::serve_graphql(schema.schema().clone());

    let sse_subject = Subject::from_str("vehicle.events").expect("valid subject");
    let sse_router = serve_sse_with_publisher(Arc::clone(&publisher), sse_subject);

    let app = graphql_router.merge(sse_router);

    let body = serde_json::json!({
        "query": r#"{ resource(subject: "vehicle.sensors.imu", selection: "all") }"#
    });
    let response = tower::ServiceExt::oneshot(
        app.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/graphql")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap(),
    )
    .await
    .expect("router responds");

    assert_eq!(
        response.status(),
        axum::http::StatusCode::OK,
        "POST /graphql should return 200"
    );
}

#[tokio::test]
async fn serve_builds_router_with_sse_endpoint() {
    let publisher = Arc::new(TokioSsePublisher::new());
    let sse_subject = Subject::from_str("vehicle.events").expect("valid subject");
    let sse_router = serve_sse_with_publisher(Arc::clone(&publisher), sse_subject);

    let response = tower::ServiceExt::oneshot(
        sse_router,
        axum::http::Request::builder()
            .method("GET")
            .uri("/events")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await
    .expect("router responds");

    assert_eq!(
        response.status(),
        axum::http::StatusCode::OK,
        "GET /events should return 200"
    );
}
