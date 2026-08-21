//! Integration test: SledStorage round-trip put/get/query/delete.
//!
//! Covers `SPEC.toml [quality] integration_tests_required = true`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use themql_core::FormatTag;
use themql_storage::{SledStorage, Storage, StorageKey, StorageQuery, StorageValue};

fn sample_value(payload: &[u8]) -> StorageValue {
    StorageValue {
        bytes: payload.to_vec(),
        format: FormatTag::Json,
    }
}

#[tokio::test]
async fn sled_put_get_round_trip() {
    let storage = SledStorage::open_temp().expect("sled temp");
    let key = StorageKey::new("alpha.beta");
    let value = sample_value(b"{\"x\":1}");
    storage.put(&key, value.clone()).await.expect("put");
    let got = storage.get(&key).await.expect("get");
    assert_eq!(got, Some(value));
}

#[tokio::test]
async fn sled_delete_removes_value() {
    let storage = SledStorage::open_temp().expect("sled temp");
    let key = StorageKey::new("delete.me");
    storage.put(&key, sample_value(b"v")).await.expect("put");
    storage.delete(&key).await.expect("delete");
    assert!(storage.get(&key).await.expect("get").is_none());
}

#[tokio::test]
async fn sled_get_missing_returns_none() {
    let storage = SledStorage::open_temp().expect("sled temp");
    let key = StorageKey::new("never.inserted");
    assert!(storage.get(&key).await.expect("get").is_none());
}

#[tokio::test]
async fn sled_query_by_key_returns_matching_entries() {
    let storage = SledStorage::open_temp().expect("sled temp");
    storage
        .put(
            &StorageKey::new("vehicle.sensors.imu"),
            sample_value(b"imu"),
        )
        .await
        .expect("put imu");
    storage
        .put(
            &StorageKey::new("vehicle.sensors.gps"),
            sample_value(b"gps"),
        )
        .await
        .expect("put gps");

    let results = storage
        .query(&StorageQuery::ByKey(StorageKey::new("vehicle.sensors.imu")))
        .await
        .expect("query");
    assert_eq!(results.entries.len(), 1);
    assert_eq!(results.entries[0].0.as_str(), "vehicle.sensors.imu");
}

#[tokio::test]
async fn sled_overwrite_existing_key() {
    let storage = SledStorage::open_temp().expect("sled temp");
    let key = StorageKey::new("overwrite");
    storage
        .put(&key, sample_value(b"v1"))
        .await
        .expect("put v1");
    storage
        .put(&key, sample_value(b"v2"))
        .await
        .expect("put v2");
    let got = storage.get(&key).await.expect("get").expect("some");
    assert_eq!(got.bytes, b"v2");
}
