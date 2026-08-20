//! Shared sensor helper types used by driver submodules.
//!
//! The [`crate::SensorDriver`] trait, [`crate::SensorError`],
//! [`crate::SensorReading`], [`crate::SensorKind`], and
//! [`crate::HeapString`] are defined in the crate root (`main.rs`) and
//! re-exported here for convenience in driver modules.

pub use crate::{HeapString, SensorDriver, SensorError, SensorKind, SensorReading};

/// Construct a [`HeapString`] from a `&str` — convenience for drivers.
#[must_use]
pub fn heap_string(s: &str) -> HeapString {
    HeapString::new(s)
}
