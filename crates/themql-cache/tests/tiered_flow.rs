//! Integration test: tiered cache flow L1 → L2 → L4 (sled) with
//! write-through, promote-on-hit, and invalidate.
//!
//! Covers `SPEC.toml [quality] integration_tests_required = true`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use themql_cache::{
    Cache, CacheEntry, CacheHit, CacheKey, CachePolicy, CacheTier, L1Cache, L2Cache, TieredCache,
};
use themql_core::{ResponseValue, Timestamp};
use themql_storage::SledStorage;

fn sample_entry(tier: CacheTier) -> CacheEntry {
    CacheEntry {
        value: ResponseValue::Json(serde_json::json!({"k": "v"})),
        inserted_at: Timestamp::now_monotonic(),
        ttl: Some(Duration::from_secs(60)),
        source_tier: tier,
        etag: None,
    }
}

#[tokio::test]
async fn tiered_l4_sled_promotes_to_l1_on_hit() {
    let l4 = SledStorage::open_temp().expect("sled temp");
    let l1 = Some(L1Cache::new(8));
    let l2 = Some(L2Cache::new(8));
    let cache = TieredCache::new(l1, l2, None, l4);

    let key = CacheKey::hash_of(b"integration-l4");
    let entry = sample_entry(CacheTier::L4);

    cache
        .put(key.clone(), entry.clone(), &CachePolicy::default())
        .await
        .expect("put");

    cache.l1.as_ref().unwrap().invalidate(&key);
    cache.l2.as_ref().unwrap().invalidate(&key);

    let hit = cache.get(&key, &CachePolicy::default()).await.expect("get");
    match hit {
        CacheHit::Hit { source_tier, .. } => assert_eq!(source_tier, CacheTier::L4),
        CacheHit::Miss => panic!("expected L4 hit"),
    }
    assert!(
        cache.l1.as_ref().unwrap().get(&key).is_some(),
        "L4 hit should promote to L1"
    );
    assert!(
        cache.l2.as_ref().unwrap().get(&key).is_some(),
        "L4 hit should promote to L2"
    );
}

#[tokio::test]
async fn tiered_invalidate_removes_from_l1_l2_and_l4() {
    let l4 = SledStorage::open_temp().expect("sled temp");
    let l1 = Some(L1Cache::new(8));
    let l2 = Some(L2Cache::new(8));
    let cache = TieredCache::new(l1, l2, None, l4);

    let key = CacheKey::hash_of(b"integration-invalidate");
    cache
        .put(
            key.clone(),
            sample_entry(CacheTier::L1),
            &CachePolicy::default(),
        )
        .await
        .expect("put");

    cache.invalidate(&key).await.expect("invalidate");

    assert!(cache.l1.as_ref().unwrap().get(&key).is_none(), "L1 empty");
    assert!(cache.l2.as_ref().unwrap().get(&key).is_none(), "L2 empty");
    let miss = cache.get(&key, &CachePolicy::default()).await.expect("get");
    assert!(matches!(miss, CacheHit::Miss), "L4 miss after invalidate");
}

#[tokio::test]
async fn tiered_disabled_policy_returns_miss_without_lookup() {
    let l4 = SledStorage::open_temp().expect("sled temp");
    let l1 = Some(L1Cache::new(8));
    let cache = TieredCache::new(l1, None, None, l4);

    let key = CacheKey::hash_of(b"disabled-policy");
    cache
        .l1
        .as_ref()
        .unwrap()
        .put(key.clone(), sample_entry(CacheTier::L1));

    let hit = cache
        .get(&key, &CachePolicy::disabled())
        .await
        .expect("get");
    assert!(matches!(hit, CacheHit::Miss), "disabled policy must miss");
}

#[tokio::test]
async fn tiered_bypass_policy_skips_read_but_writes_through() {
    let l4 = SledStorage::open_temp().expect("sled temp");
    let l1 = Some(L1Cache::new(8));
    let cache = TieredCache::new(l1, None, None, l4);

    let key = CacheKey::hash_of(b"bypass-policy");
    let bypass_policy = CachePolicy {
        bypass: true,
        ..CachePolicy::default()
    };

    let hit = cache.get(&key, &bypass_policy).await.expect("get");
    assert!(matches!(hit, CacheHit::Miss), "bypass must miss");

    cache
        .put(key.clone(), sample_entry(CacheTier::L1), &bypass_policy)
        .await
        .expect("put writes through even on bypass");

    let normal_policy = CachePolicy::default();
    let hit_after = cache.get(&key, &normal_policy).await.expect("get");
    assert!(
        matches!(hit_after, CacheHit::Hit { .. }),
        "after bypass write, normal read should hit"
    );
}
