//! Benchmark: GraphQL query resolution through the resolver bridge.
//!
//! Covers `SPEC.toml [quality] benchmark_hot_paths = true`.

use std::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use themql_core::{Context, CorrelationId, Error, Query, ResolverBoxed, Response, ResponseValue};
use themql_graphql::{
    AuthRole, DispatchBridgeImpl, GraphqlResolverBridgeImpl, GraphqlSchema, GraphqlSchemaImpl,
    MutationRoot, QueryRoot, SubscriptionRoot,
};

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

impl themql_core::MessageHandler for StubHandler {
    fn handle<'a>(
        &'a self,
        msg: &'a themql_core::Message,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, Error>> + Send + 'a>>
    {
        let subject = msg.subject.as_str();
        Box::pin(async move {
            Ok(Response::ok(
                ResponseValue::Json(serde_json::json!({"subject": subject})),
                CorrelationId::new(),
            ))
        })
    }
}

fn bench_graphql_resource_query(c: &mut Criterion) {
    let resolver: Arc<dyn themql_core::Resolver> = Arc::new(StubResolver);
    let bridge = Arc::new(GraphqlResolverBridgeImpl::new(resolver));
    let handler: Arc<dyn themql_core::MessageHandler> = Arc::new(StubHandler);
    let dispatch = Arc::new(DispatchBridgeImpl::new(handler));
    let schema = GraphqlSchemaImpl::with_role(
        QueryRoot::new(bridge),
        MutationRoot::new(dispatch),
        SubscriptionRoot::default(),
        AuthRole::Observer,
    );

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    c.bench_function("graphql_resource_query", |b| {
        b.to_async(&rt).iter(|| async {
            let q = r#"{ resource(subject: "vehicle.sensors.imu", selection: "all") }"#;
            let _ = black_box(schema.schema().execute(q).await);
        });
    });
}

criterion_group!(benches, bench_graphql_resource_query);
criterion_main!(benches);
