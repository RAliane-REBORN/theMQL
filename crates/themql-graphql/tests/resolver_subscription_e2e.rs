//! Integration test: GraphQL resolver query + subscription end-to-end.
//!
//! Covers `SPEC.toml [quality] integration_tests_required = true`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use futures_util::StreamExt;
use themql_core::{
    Context, CorrelationId, Error, Message, MessageHandler, Operation, Payload, Query,
    ResolverBoxed, Response, ResponseValue, Subject,
};
use themql_graphql::{
    AuthRole, DispatchBridge, DispatchBridgeImpl, GraphqlResolverBridge, GraphqlResolverBridgeImpl,
    GraphqlSchema, GraphqlSchemaImpl, GraphqlSubscriptionSource, MutationRoot, QueryRoot,
    SubscriptionRoot,
};
use themql_sse::{SsePublisher, TokioSsePublisher};

use std::future::Future;
use std::pin::Pin;

struct StubResolver;

#[allow(clippy::unused_async_trait_impl)]
impl ResolverBoxed for StubResolver {
    async fn resolve(&self, query: &Query, _ctx: &Context) -> Result<Response, Error> {
        let subject = query.resource.subject.as_str();
        Ok(Response::ok(
            ResponseValue::Json(serde_json::json!({"subject": subject})),
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

#[tokio::test]
async fn graphql_query_resolver_end_to_end() {
    let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
    let bridge: Arc<dyn GraphqlResolverBridge> = Arc::new(GraphqlResolverBridgeImpl::new(resolver));
    let dispatch: Arc<dyn DispatchBridge> =
        Arc::new(DispatchBridgeImpl::new(Arc::new(StubHandler)));
    let schema = GraphqlSchemaImpl::with_role(
        QueryRoot::new(bridge),
        MutationRoot::new(dispatch),
        SubscriptionRoot::default(),
        AuthRole::Observer,
    );

    let q = r#"{ resource(subject: "vehicle.sensors.imu", selection: "all") }"#;
    let response = schema.schema().execute(q).await;
    assert!(
        response.errors.is_empty(),
        "query errors: {:?}",
        response.errors
    );
    let json: serde_json::Value = response.data.into_json().expect("data to json");
    assert!(
        json.get("resource").is_some(),
        "data should contain resource: {json}"
    );
}

#[tokio::test]
async fn graphql_subscription_streams_events_from_sse_publisher() {
    let publisher = Arc::new(TokioSsePublisher::new());
    let source: Arc<dyn GraphqlSubscriptionSource> =
        Arc::clone(&publisher) as Arc<dyn GraphqlSubscriptionSource>;
    let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
    let bridge: Arc<dyn GraphqlResolverBridge> = Arc::new(GraphqlResolverBridgeImpl::new(resolver));
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
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let subject = Subject::from_str("vehicle.state_estimate").expect("subject");
        let subject_str = subject.as_str();
        let msg = Message::new(&subject_str, Operation::Event).expect("message");
        publisher_clone
            .broadcast(&subject, &msg)
            .await
            .expect("broadcast");
    });

    let q = r#"subscription { subscribe(subject: "vehicle.state_estimate") }"#;
    let mut stream = schema.schema().execute_stream(q);
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next()).await;
    assert!(result.is_ok(), "subscription should produce an event");
    let response = result.expect("timeout").expect("event");
    assert!(
        response.errors.is_empty(),
        "no errors expected, got {response:?}"
    );
}
