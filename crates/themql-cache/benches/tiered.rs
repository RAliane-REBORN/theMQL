//! Benchmark: tiered cache get/put hot paths (L1/L2/L4).
//!
//! Covers `SPEC.toml [quality] benchmark_hot_paths = true`.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use themql_cache::{
    Cache, CacheEntry, CacheKey, CachePolicy, CacheTier, L1Cache, L2Cache, TieredCache,
};
use themql_core::{ResponseValue, Timestamp};
use themql_storage::SledStorage;

fn sample_entry() -> CacheEntry {
    CacheEntry {
        value: ResponseValue::Json(serde_json::json!({"k": "v"})),
        inserted_at: Timestamp::now_monotonic(),
        ttl: None,
        source_tier: CacheTier::L1,
        etag: None,
    }
}

fn bench_l1_get_hit(c: &mut Criterion) {
    c.bench_function("l1_get_hit", |b| {
        let l1 = L1Cache::new(1024);
        let key = CacheKey::hash_of(b"bench-l1");
        l1.put(key.clone(), sample_entry());
        b.iter(|| {
            let _ = black_box(l1.get(&key));
        });
    });
}

fn bench_l1_put(c: &mut Criterion) {
    c.bench_function("l1_put", |b| {
        let l1 = L1Cache::new(1024);
        let key = CacheKey::hash_of(b"bench-l1-put");
        let entry = sample_entry();
        b.iter(|| {
            l1.put(black_box(key.clone()), black_box(entry.clone()));
        });
    });
}

fn bench_l2_get_hit(c: &mut Criterion) {
    c.bench_function("l2_get_hit", |b| {
        let l2 = L2Cache::new(1024);
        let key = CacheKey::hash_of(b"bench-l2");
        l2.put(key.clone(), sample_entry());
        b.iter(|| {
            let _ = black_box(l2.get(&key));
        });
    });
}

fn bench_tiered_l1_hit_async(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    c.bench_function("tiered_l1_hit_async", |b| {
        b.to_async(&rt).iter(|| async {
            let l1 = Some(L1Cache::new(1024));
            let l4 = SledStorage::open_temp().expect("sled");
            let cache = TieredCache::new(l1, None, None, l4);
            let key = CacheKey::hash_of(b"bench-tiered-l1");
            cache.l1.as_ref().unwrap().put(key.clone(), sample_entry());
            let _ = black_box(cache.get(&key, &CachePolicy::default()).await.expect("get"));
        });
    });
}

fn bench_tiered_l4_sled_hit_async(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    c.bench_function("tiered_l4_sled_hit_async", |b| {
        b.to_async(&rt).iter(|| async {
            let l4 = SledStorage::open_temp().expect("sled");
            let cache = TieredCache::new(None, None, None, l4);
            let key = CacheKey::hash_of(b"bench-tiered-l4");
            cache
                .put(key.clone(), sample_entry(), &CachePolicy::default())
                .await
                .expect("put");
            let _ = black_box(cache.get(&key, &CachePolicy::default()).await.expect("get"));
        });
    });
}

criterion_group!(
    benches,
    bench_l1_get_hit,
    bench_l1_put,
    bench_l2_get_hit,
    bench_tiered_l1_hit_async,
    bench_tiered_l4_sled_hit_async
);
criterion_main!(benches);
