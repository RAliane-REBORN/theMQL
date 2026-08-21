//! Property tests for `Subject` / `SubjectPattern` grammar and matching.
//!
//! Covers `SPEC.toml [quality] property_tests_required = true`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use themql_core::{Subject, SubjectPattern};

fn valid_segment_regex() -> impl Strategy<Value = String> {
    r"[a-zA-Z0-9_\-]{1,8}"
}

fn subject_regex() -> impl Strategy<Value = String> {
    prop::collection::vec(valid_segment_regex(), 1..5).prop_map(|segs| segs.join("."))
}

proptest! {
    #[test]
    fn subject_round_trips_through_string(s in subject_regex()) {
        let subj = Subject::from_str(&s).expect("valid subject");
        prop_assert_eq!(subj.as_str(), s);
    }

    #[test]
    fn subject_clone_equals_original(s in subject_regex()) {
        let subj = Subject::from_str(&s).expect("valid");
        let cloned = subj.clone();
        prop_assert_eq!(&subj, &cloned);
    }

    #[test]
    fn subject_is_always_concrete(s in subject_regex()) {
        let subj = Subject::from_str(&s).expect("valid");
        prop_assert!(subj.is_concrete());
    }

    #[test]
    fn subject_display_equals_as_str(s in subject_regex()) {
        let subj = Subject::from_str(&s).expect("valid");
        prop_assert_eq!(format!("{subj}"), subj.as_str());
    }

    #[test]
    fn pattern_matches_itself_as_concrete(s in subject_regex()) {
        let subj = Subject::from_str(&s).expect("valid subject");
        let pat = SubjectPattern::from_str(&s).expect("valid pattern");
        prop_assert!(pat.matches(&subj));
    }

    #[test]
    fn wildcard_multi_matches_any_subpath(prefix in subject_regex(), suffix in subject_regex()) {
        let pat_str = format!("{prefix}.#");
        let pat = SubjectPattern::from_str(&pat_str).expect("valid pattern");
        let subj = Subject::from_str(&suffix).expect("valid subject");
        let pat_matches = pat.matches(&subj);
        let prefix_is_prefix = suffix.starts_with(&format!("{prefix}."));
        prop_assert_eq!(pat_matches, prefix_is_prefix || prefix == suffix);
    }

    #[test]
    fn wildcard_one_matches_exactly_one_segment(
        prefix in valid_segment_regex(),
        middle in valid_segment_regex(),
        suffix in valid_segment_regex()
    ) {
        let pat_str = format!("{prefix}.+.{suffix}");
        let pat = SubjectPattern::from_str(&pat_str).expect("valid pattern");
        let subj_str = format!("{prefix}.{middle}.{suffix}");
        let subj = Subject::from_str(&subj_str).expect("valid subject");
        prop_assert!(pat.matches(&subj));
        let wrong_subj_str = format!("{prefix}.{middle}.{middle}.{suffix}");
        let wrong_subj = Subject::from_str(&wrong_subj_str).expect("valid");
        prop_assert!(!pat.matches(&wrong_subj));
    }
}
