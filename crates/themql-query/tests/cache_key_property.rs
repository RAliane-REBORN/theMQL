//! Property tests for `CacheKey` determinism + `CacheKeyer` collisions.
//!
//! Covers `SPEC.toml [quality] property_tests_required = true`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use themql_core::{Query, Resource, Subject};
use themql_query::{CacheKey, CacheKeyer, DefaultCacheKeyer};

fn subject_regex() -> impl Strategy<Value = String> {
    prop::collection::vec(r"[a-z]{1,5}", 1..3).prop_map(|segs| segs.join("."))
}

fn query_for(subject_str: String) -> Query {
    let subj = Subject::from_str(&subject_str).expect("valid subject");
    Query::new(Resource::from_subject(subj))
}

proptest! {
    #[test]
    fn cache_keyer_is_deterministic(s in subject_regex()) {
        let keyer = DefaultCacheKeyer::new();
        let q1 = query_for(s.clone());
        let q2 = query_for(s);
        prop_assert_eq!(keyer.key(&q1), keyer.key(&q2));
    }

    #[test]
    fn cache_keyer_distinct_for_distinct_subjects(
        s1 in subject_regex(),
        s2 in subject_regex()
    ) {
        prop_assume!(s1 != s2);
        let keyer = DefaultCacheKeyer::new();
        let q1 = query_for(s1);
        let q2 = query_for(s2);
        prop_assert_ne!(keyer.key(&q1), keyer.key(&q2));
    }

    #[test]
    fn cache_key_hash_of_is_deterministic(input in prop::collection::vec(any::<u8>(), 0..64)) {
        let k1 = CacheKey::hash_of(&input);
        let k2 = CacheKey::hash_of(&input);
        prop_assert_eq!(k1, k2);
    }

    #[test]
    fn cache_key_hash_of_distinct_for_distinct_input(
        a in prop::collection::vec(any::<u8>(), 1..32),
        b in prop::collection::vec(any::<u8>(), 1..32)
    ) {
        prop_assume!(a != b);
        prop_assert_ne!(CacheKey::hash_of(&a), CacheKey::hash_of(&b));
    }

    #[test]
    fn cache_key_display_is_hex_64_chars(input in prop::collection::vec(any::<u8>(), 1..32)) {
        let k = CacheKey::hash_of(&input);
        let s = format!("{k}");
        prop_assert_eq!(s.len(), 64);
        for c in s.chars() {
            prop_assert!(c.is_ascii_hexdigit());
        }
    }
}
