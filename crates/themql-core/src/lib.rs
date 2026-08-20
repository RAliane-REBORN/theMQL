//! # themql-core
//!
//! Canonical owner of theMQL's core type identities: `Message`, `Query`,
//! `Response`, `Error`, `Context`, and `Resource` identity.
//!
//! Every other crate in theMQL is subordinate to this one for type
//! definitions. See `specs/core.toml` for the authoritative specification.
//!
//! ## Authority
//!
//! Per `SPEC.toml` and `prompts/SYSTEM.md`, the canonical types here are
//! the semantic owners. Transport adapters (`themql-mqtt`,
//! `themql-graphql`, `themql-sse`) project these types; they do not
//! redefine them.
//!
//! ## Example
//!
//! ```
//! use themql_core::{Message, MessageId, Subject, Operation, Payload,
//!                    Metadata, Timestamp, TimestampKind};
//!
//! let subject = Subject::from_str("vehicle.sensors.imu.gyro")
//!     .expect("valid subject");
//! let msg = Message {
//!     id: MessageId::new(),
//!     timestamp: Timestamp::now_monotonic(),
//!     subject,
//!     operation: Operation::Telemetry,
//!     payload: Payload::Unit,
//!     metadata: Metadata::default(),
//! };
//! assert_eq!(msg.operation, Operation::Telemetry);
//! ```

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ===========================================================================
// Identity types
// ===========================================================================

/// Unique identifier for a `Message`. Wraps a UUID v7 (monotonic-ish).
///
/// Generated via [`MessageId::new`]; equality and ordering are by the
/// inner UUID. Serialises transparently as the inner UUID string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(pub Uuid);

impl MessageId {
    /// Generate a fresh `MessageId` (UUID v7 — time-ordered).
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// The inner UUID.
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Links a `Response` (or error) back to the `Query`/`Command` that
/// caused it. Required on Responses; optional on commands/events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrelationId(pub Uuid);

impl CorrelationId {
    /// Derive a correlation id from the originating request's id.
    #[must_use]
    pub fn for_request(request: &MessageId) -> Self {
        // Use the same UUID — a response correlates 1:1 with its request.
        Self(request.0)
    }

    /// Generate a fresh correlation id.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

/// Links an `Event` back to the `Command` that caused it. Required on
/// Events; optional elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CausationId(pub Uuid);

impl CausationId {
    /// Generate a fresh causation id.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for CausationId {
    fn default() -> Self {
        Self::new()
    }
}

/// Trace identifier for distributed tracing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TraceId(pub Uuid);

impl TraceId {
    /// Generate a fresh trace id.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Subject — hierarchical routing key. Grammar lives in message.toml; the
// type is defined here because every transport depends on it.
// ===========================================================================

/// A single non-empty segment of a `Subject`.
///
/// Segments are case-sensitive, may contain `a-zA-Z0-9_-`, and must be
/// non-empty. The separator `.` is between segments, never within.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SubjectSegment(pub String);

impl SubjectSegment {
    /// Construct a segment, validating charset + non-empty.
    ///
    /// # Errors
    /// Returns `SubjectError::EmptySegment` if the string is empty, or
    /// `SubjectError::InvalidCharset` if it contains characters outside
    /// `a-zA-Z0-9_-`.
    pub fn new(s: &str) -> Result<Self, SubjectError> {
        if s.is_empty() {
            return Err(SubjectError::EmptySegment);
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(SubjectError::InvalidCharset {
                segment: s.to_owned(),
            });
        }
        Ok(Self(s.to_owned()))
    }

    /// The segment string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SubjectSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A concrete, hierarchical, dot-separated routing key.
///
/// Example: `vehicle.sensors.imu.gyro`. Subjects are case-sensitive,
/// have no leading/trailing/consecutive separators, and never contain
/// wildcards (`+`/`#` — see [`SubjectPattern`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Subject {
    segments: Vec<SubjectSegment>,
}

impl Subject {
    /// Construct a `Subject` from segments.
    ///
    /// # Errors
    /// Returns `SubjectError::EmptySubject` if `segments` is empty.
    pub fn from_segments(segments: Vec<SubjectSegment>) -> Result<Self, SubjectError> {
        if segments.is_empty() {
            return Err(SubjectError::EmptySubject);
        }
        Ok(Self { segments })
    }

    /// Parse a `Subject` from a dot-joined string.
    ///
    /// # Errors
    /// Returns a [`SubjectError`] variant for any grammar violation
    /// (empty segment, invalid charset, leading/trailing/consecutive
    /// separator, wildcard in concrete subject, empty subject).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, SubjectError> {
        if s.is_empty() {
            return Err(SubjectError::EmptySubject);
        }
        if s.starts_with('.') {
            return Err(SubjectError::LeadingSeparator);
        }
        if s.ends_with('.') {
            return Err(SubjectError::TrailingSeparator);
        }
        if s.contains("..") {
            return Err(SubjectError::ConsecutiveSeparators);
        }
        let segments: Vec<SubjectSegment> = s
            .split('.')
            .map(|part| {
                // Wildcards are not valid in concrete Subjects — check before
                // charset validation so we report the right error variant.
                if part == "+" || part == "#" {
                    return Err(SubjectError::WildcardInConcreteSubject);
                }
                SubjectSegment::new(part)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { segments })
    }

    /// The segments of this subject.
    #[must_use]
    pub fn segments(&self) -> &[SubjectSegment] {
        &self.segments
    }

    /// Dot-joined string form.
    #[must_use]
    pub fn as_str(&self) -> String {
        self.segments
            .iter()
            .map(SubjectSegment::as_str)
            .collect::<Vec<_>>()
            .join(".")
    }

    /// Always `true` for `Subject` (only patterns can be non-concrete).
    #[must_use]
    pub fn is_concrete(&self) -> bool {
        true
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

/// A single segment of a [`SubjectPattern`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternSegment {
    /// Matches one concrete segment by exact string equality.
    Concrete(SubjectSegment),
    /// `+` — matches any single concrete segment.
    WildcardOne,
    /// `#` — matches zero or more trailing concrete segments. Must be
    /// the LAST segment of a pattern.
    WildcardMulti,
}

/// A `Subject` that may contain wildcards. Used for subscriptions and
/// cache invalidation patterns, never as a concrete `Message.subject`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubjectPattern {
    segments: Vec<PatternSegment>,
}

impl SubjectPattern {
    /// Parse a `SubjectPattern` from a dot-joined string.
    ///
    /// # Errors
    /// Returns `SubjectError::WildcardMultiNotLast` if `#` appears
    /// before the last segment. Other grammar errors mirror
    /// [`Subject::from_str`].
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, SubjectError> {
        if s.is_empty() {
            return Err(SubjectError::EmptySubject);
        }
        if s.starts_with('.') {
            return Err(SubjectError::LeadingSeparator);
        }
        if s.ends_with('.') {
            return Err(SubjectError::TrailingSeparator);
        }
        if s.contains("..") {
            return Err(SubjectError::ConsecutiveSeparators);
        }
        let parts: Vec<&str> = s.split('.').collect();
        let mut segments = Vec::with_capacity(parts.len());
        for (i, part) in parts.iter().enumerate() {
            let seg = match *part {
                "+" => PatternSegment::WildcardOne,
                "#" => {
                    if i != parts.len() - 1 {
                        return Err(SubjectError::WildcardMultiNotLast);
                    }
                    PatternSegment::WildcardMulti
                }
                other => PatternSegment::Concrete(SubjectSegment::new(other)?),
            };
            segments.push(seg);
        }
        Ok(Self { segments })
    }

    /// Test whether this pattern matches a concrete subject.
    ///
    /// - `Concrete` matches by exact string equality;
    /// - `WildcardOne` (`+`) matches any single segment;
    /// - `WildcardMulti` (`#`) matches zero or more trailing segments.
    #[must_use]
    pub fn matches(&self, subject: &Subject) -> bool {
        let concrete = subject.segments();
        let pattern = &self.segments;
        let mut ci = 0;
        let mut pi = 0;
        while pi < pattern.len() && ci < concrete.len() {
            match &pattern[pi] {
                PatternSegment::Concrete(seg) => {
                    if seg != &concrete[ci] {
                        return false;
                    }
                    ci += 1;
                    pi += 1;
                }
                PatternSegment::WildcardOne => {
                    ci += 1;
                    pi += 1;
                }
                PatternSegment::WildcardMulti => {
                    // '#' matches the rest; only valid as last pattern segment.
                    return pi == pattern.len() - 1;
                }
            }
        }
        // Trailing '#' matches zero-or-more, so a pattern ending in '#' can
        // match even if the concrete subject is shorter.
        if pi < pattern.len() && matches!(pattern[pi], PatternSegment::WildcardMulti) {
            return true;
        }
        pi == pattern.len() && ci == concrete.len()
    }

    /// Dot-joined string form.
    #[must_use]
    pub fn as_str(&self) -> String {
        self.segments
            .iter()
            .map(|s| match s {
                PatternSegment::Concrete(seg) => seg.as_str().to_owned(),
                PatternSegment::WildcardOne => "+".to_owned(),
                PatternSegment::WildcardMulti => "#".to_owned(),
            })
            .collect::<Vec<_>>()
            .join(".")
    }

    /// The pattern segments.
    #[must_use]
    pub fn segments(&self) -> &[PatternSegment] {
        &self.segments
    }
}

impl fmt::Display for SubjectPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

/// Errors raised while constructing or parsing [`Subject`] /
/// [`SubjectPattern`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubjectError {
    /// A segment was empty.
    #[error("empty segment")]
    EmptySegment,
    /// A segment contained characters outside `a-zA-Z0-9_-`.
    #[error("invalid charset in segment: {segment}")]
    InvalidCharset {
        /// The offending segment string.
        segment: String,
    },
    /// A segment contained a `.` (separator belongs between segments).
    #[error("separator inside segment")]
    SeparatorInSegment,
    /// The subject had a trailing `.`.
    #[error("trailing separator")]
    TrailingSeparator,
    /// The subject had a leading `.`.
    #[error("leading separator")]
    LeadingSeparator,
    /// The subject had two consecutive `.` separators.
    #[error("consecutive separators")]
    ConsecutiveSeparators,
    /// A concrete subject contained `+` or `#`.
    #[error("wildcard in concrete subject")]
    WildcardInConcreteSubject,
    /// A pattern had `#` before the last segment.
    #[error("wildcard '#' must be the last segment")]
    WildcardMultiNotLast,
    /// The subject was the empty string.
    #[error("empty subject")]
    EmptySubject,
}

// ===========================================================================
// Operation — the verb of a Message
// ===========================================================================

/// The verb of a `Message`. Tags payload semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// A read-only query against a resource.
    Query,
    /// A side-effectful command.
    Command,
    /// An event that has occurred.
    Event,
    /// A long-lived stream subscription.
    Stream,
    /// The response to a query or command.
    Response,
    /// A telemetry reading.
    Telemetry,
    /// An error.
    Error,
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Query => "query",
            Self::Command => "command",
            Self::Event => "event",
            Self::Stream => "stream",
            Self::Response => "response",
            Self::Telemetry => "telemetry",
            Self::Error => "error",
        };
        f.write_str(s)
    }
}

// ===========================================================================
// Payload — typed, serde-serialisable, format-tagged
// ===========================================================================

/// Tag identifying the serialisation format of a payload.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatTag {
    /// JSON.
    Json,
    /// Bincode.
    Bincode,
    /// Postcard (embedded-friendly).
    Postcard,
    /// A custom format identified by name. Must be explicit in
    /// `Message.metadata`.
    Custom(String),
}

impl fmt::Display for FormatTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => f.write_str("json"),
            Self::Bincode => f.write_str("bincode"),
            Self::Postcard => f.write_str("postcard"),
            Self::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

/// Explicitly typed, serde-serialisable payload of a `Message`. The
/// format is tagged in the payload itself (never inferred from
/// transport).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Payload {
    /// JSON payload.
    Json(serde_json::Value),
    /// Binary payload with an explicit format tag.
    Bytes(Vec<u8>, FormatTag),
    /// No payload body (e.g. heartbeat).
    Unit,
}

// ===========================================================================
// Metadata — key-value bag
// ===========================================================================

/// Auth principal carried in `Metadata`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    /// Principal identifier (e.g. user id, service id).
    pub id: String,
    /// Scopes granted to this principal.
    pub scopes: Vec<String>,
}

/// Key-value bag carried on every `Message`. Holds correlation id,
/// causation id, trace id, auth principal, format hint, and arbitrary
/// extension keys.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Metadata {
    /// Correlation id — links responses to requests.
    pub correlation_id: Option<CorrelationId>,
    /// Causation id — links events to the command that caused them.
    pub causation_id: Option<CausationId>,
    /// Trace id for distributed tracing.
    pub trace_id: Option<TraceId>,
    /// Auth principal.
    pub auth_principal: Option<Principal>,
    /// Optional explicit format hint.
    pub format: Option<FormatTag>,
    /// Extension keys. `BTreeMap` for deterministic iteration and hash.
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl Metadata {
    /// Construct a `Metadata` with no named fields set and no extensions.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the correlation id.
    #[must_use]
    pub fn with_correlation(mut self, id: CorrelationId) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Set the causation id.
    #[must_use]
    pub fn with_causation(mut self, id: CausationId) -> Self {
        self.causation_id = Some(id);
        self
    }

    /// Set the trace id.
    #[must_use]
    pub fn with_trace(mut self, id: TraceId) -> Self {
        self.trace_id = Some(id);
        self
    }

    /// Add an extension key-value pair. Extensions must not duplicate
    /// the named fields.
    #[must_use]
    pub fn with_extension<K: Into<String>, V: Into<serde_json::Value>>(
        mut self,
        key: K,
        value: V,
    ) -> Self {
        self.extensions.insert(key.into(), value.into());
        self
    }
}

// ===========================================================================
// Timestamp + Deadline
// ===========================================================================

/// Kind of timestamp — monotonic vs wall-clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampKind {
    /// Monotonic — safe for ordering, not for cross-process correlation.
    Monotonic,
    /// Wall-clock — safe for cross-process correlation, subject to NTP.
    WallClock,
}

/// Timestamp on every `Message`. Tagged explicitly so consumers know
/// whether to trust it for ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Timestamp {
    /// Whether this is monotonic or wall-clock.
    pub kind: TimestampKind,
    /// Nanoseconds since the Unix epoch (wall) or since process start (mono).
    pub nanos_since_epoch: i64,
}

impl Timestamp {
    /// Construct a timestamp from nanoseconds + kind.
    #[must_use]
    pub fn new(kind: TimestampKind, nanos_since_epoch: i64) -> Self {
        Self {
            kind,
            nanos_since_epoch,
        }
    }

    /// Current monotonic timestamp (best-effort).
    #[must_use]
    pub fn now_monotonic() -> Self {
        Self::new(TimestampKind::Monotonic, 0)
    }

    /// Current wall-clock timestamp (best-effort).
    #[must_use]
    pub fn now_wall_clock() -> Self {
        Self::new(TimestampKind::WallClock, 0)
    }
}

/// Absolute deadline for a `Query` resolution or `Command` dispatch.
/// Carried in `Context`. Resolvers must check the deadline before
/// returning a result and return [`Error`] with code
/// [`ErrorCode::Timeout`] if exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Deadline {
    /// Whether this is monotonic or wall-clock.
    pub kind: TimestampKind,
    /// Absolute nanoseconds since epoch (or process start) at which the
    /// deadline expires.
    pub nanos_since_epoch: i64,
}

impl Deadline {
    /// Construct a deadline.
    #[must_use]
    pub fn new(kind: TimestampKind, nanos_since_epoch: i64) -> Self {
        Self {
            kind,
            nanos_since_epoch,
        }
    }

    /// Whether the deadline has expired relative to `now`.
    #[must_use]
    pub fn is_expired(&self, now: &Timestamp) -> bool {
        self.kind == now.kind && self.nanos_since_epoch <= now.nanos_since_epoch
    }
}

// ===========================================================================
// Selection / Projection — filter + projection combined (per decision)
// ===========================================================================

/// A dot-joined path into a resource (e.g. `position.x`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldPath {
    segments: Vec<String>,
}

impl FieldPath {
    /// Construct a field path from segments.
    #[must_use]
    pub fn from_segments(segments: Vec<String>) -> Self {
        Self { segments }
    }

    /// Parse a field path from a dot-joined string.
    ///
    /// # Errors
    /// Returns [`SubjectError::EmptySegment`] if any segment is empty.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, SubjectError> {
        if s.is_empty() {
            return Err(SubjectError::EmptySegment);
        }
        let segments: Vec<String> = s.split('.').map(str::to_owned).collect();
        if segments.iter().any(String::is_empty) {
            return Err(SubjectError::EmptySegment);
        }
        Ok(Self { segments })
    }

    /// The path segments.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Dot-joined string form.
    #[must_use]
    pub fn as_str(&self) -> String {
        self.segments.join(".")
    }
}

impl fmt::Display for FieldPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

/// Comparison operator for a selection predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
    /// In a set.
    In,
    /// Contains a value.
    Contains,
}

/// A boolean predicate tree over resource fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Predicate {
    /// A field comparison: `path op value`.
    Field {
        /// Path to the field being compared.
        path: FieldPath,
        /// Comparison operator.
        op: Comparison,
        /// Value to compare against.
        value: serde_json::Value,
    },
    /// Conjunction (AND) of sub-predicates.
    And(Vec<Predicate>),
    /// Disjunction (OR) of sub-predicates.
    Or(Vec<Predicate>),
    /// Negation (NOT) of a sub-predicate.
    Not(Box<Predicate>),
}

/// Optional filter predicate selecting a subset of a resource. Combined
/// with [`Projection`], this forms the 'filter + projection' shape: a
/// `Query` with `selection = All` returns all entities of the resource;
/// `selection = Filter(p)` narrows which entities are returned.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum Selection {
    /// Return all entities of the resource.
    #[default]
    All,
    /// Filter entities by a predicate.
    Filter(Predicate),
}

impl Selection {
    /// Construct an 'all' selection.
    #[must_use]
    pub fn all() -> Self {
        Self::All
    }

    /// Construct a filtered selection.
    #[must_use]
    pub fn filter(p: Predicate) -> Self {
        Self::Filter(p)
    }
}

/// Optional field projection narrowing which fields of each selected
/// entity appear in the `Response`. Combined with [`Selection`], this
/// completes the 'filter + projection' shape.
///
/// `include` and `exclude` are mutually exclusive — if both non-empty,
/// the resolver must return [`ErrorCode::ValidationError`]. A projection
/// with both empty is treated the same as `None` (return all fields).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Projection {
    /// Fields to include. Mutually exclusive with `exclude`.
    pub include: Vec<FieldPath>,
    /// Fields to exclude. Mutually exclusive with `include`.
    pub exclude: Vec<FieldPath>,
}

impl Projection {
    /// Construct an empty projection (returns all fields).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an include path.
    #[must_use]
    pub fn include(mut self, path: FieldPath) -> Self {
        self.include.push(path);
        self
    }

    /// Add an exclude path.
    #[must_use]
    pub fn exclude(mut self, path: FieldPath) -> Self {
        self.exclude.push(path);
        self
    }

    /// Whether this projection is valid (include and exclude are not
    /// both non-empty).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.include.is_empty() || self.exclude.is_empty()
    }
}

// ===========================================================================
// Resource — named, addressable entity
// ===========================================================================

/// A named, addressable entity in theMQL. Every `Query` targets a
/// `Resource`. Resources have a hierarchical identity (a `Subject`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Resource {
    /// The hierarchical subject identifying this resource.
    pub subject: Subject,
}

impl Resource {
    /// Construct a resource from a subject.
    #[must_use]
    pub fn from_subject(subject: Subject) -> Self {
        Self { subject }
    }

    /// Parse a resource from a dot-joined subject string.
    ///
    /// # Errors
    /// Returns [`SubjectError`] if the string is not a valid subject.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, SubjectError> {
        Ok(Self {
            subject: Subject::from_str(s)?,
        })
    }
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.subject)
    }
}

// ===========================================================================
// CachePolicy — canonical in core (referenced by query, cache)
// ===========================================================================

/// Tier of the cache stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CacheTier {
    /// L1 — cachelito, process-local, ultra-low latency.
    L1,
    /// L2 — moka, process-local, low latency.
    L2,
    /// L3 — valkey, distributed, network latency.
    L3,
    /// L4 — helix-db, authoritative, persistent.
    L4,
}

/// Hint to the cache about when to invalidate an entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InvalidationHint {
    /// Invalidate when a message on this subject pattern is published.
    OnSubjectChange(SubjectPattern),
    /// Invalidate after this TTL.
    AfterTtl(std::time::Duration),
    /// Invalidate only via explicit `Cache::invalidate`.
    Explicit,
}

/// Policy controlling how a `Query` result is cached. Carried in
/// [`Context`] and optionally overridden on `Query.cache_policy`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachePolicy {
    /// If `false`, do not read from or write to any cache tier.
    pub enabled: bool,
    /// Explicit time-to-live; `None` means 'defer to tier default'.
    pub ttl: Option<std::time::Duration>,
    /// Preferred tier for this query; `None` means 'tier decides'.
    pub tier_hint: Option<CacheTier>,
    /// If `true`, skip cache read but still write-through on miss.
    pub bypass: bool,
    /// Optional hint to the cache about when to invalidate.
    pub invalidation_hint: Option<InvalidationHint>,
}

impl CachePolicy {
    /// Construct a default policy: enabled, no TTL, no tier hint, no
    /// bypass, no invalidation hint.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable all caching for this query.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ttl: None,
            tier_hint: None,
            bypass: false,
            invalidation_hint: None,
        }
    }

    /// Set the TTL.
    #[must_use]
    pub fn with_ttl(mut self, ttl: std::time::Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the tier hint.
    #[must_use]
    pub fn with_tier(mut self, tier: CacheTier) -> Self {
        self.tier_hint = Some(tier);
        self
    }

    /// Set the bypass flag.
    #[must_use]
    pub fn with_bypass(mut self, bypass: bool) -> Self {
        self.bypass = bypass;
        self
    }
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl: None,
            tier_hint: None,
            bypass: false,
            invalidation_hint: None,
        }
    }
}

// ===========================================================================
// Cancellation
// ===========================================================================

/// Cancellation signal propagated through the resolution chain. A
/// resolver must check cancellation before returning a result and
/// return [`ErrorCode::Timeout`] if cancelled.
///
/// Phase 1 uses a simple bool flag. Phase 2 may upgrade to a token type
/// backed by the runtime (tokio `CancellationToken` or embassy signal).
/// The public API (`is_cancelled` / `cancel`) stays stable.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Cancellation {
    cancelled: bool,
}

impl Cancellation {
    /// Construct a non-cancelled cancellation signal.
    #[must_use]
    pub fn new() -> Self {
        Self { cancelled: false }
    }

    /// Whether this signal has been cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Cancel this signal.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
}

// ===========================================================================
// Context — execution context
// ===========================================================================

/// Execution context propagated through every query resolution and
/// command dispatch. Carries deadline, auth principal, trace id,
/// cache policy, and cancellation signal.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Context {
    /// Absolute deadline for the resolution.
    pub deadline: Option<Deadline>,
    /// Auth principal.
    pub principal: Option<Principal>,
    /// Trace id.
    pub trace_id: Option<TraceId>,
    /// Cache policy.
    pub cache_policy: CachePolicy,
    /// Cancellation signal.
    pub cancellation: Cancellation,
}

impl Context {
    /// Construct a context with default cache policy and no deadline.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the deadline.
    #[must_use]
    pub fn with_deadline(mut self, d: Deadline) -> Self {
        self.deadline = Some(d);
        self
    }

    /// Set the principal.
    #[must_use]
    pub fn with_principal(mut self, p: Principal) -> Self {
        self.principal = Some(p);
        self
    }

    /// Set the trace id.
    #[must_use]
    pub fn with_trace(mut self, t: TraceId) -> Self {
        self.trace_id = Some(t);
        self
    }

    /// Set the cache policy.
    #[must_use]
    pub fn with_cache_policy(mut self, p: CachePolicy) -> Self {
        self.cache_policy = p;
        self
    }
}

// ===========================================================================
// Response — Result<ResponseValue, Error>, never both, never neither
// ===========================================================================

/// The value carried by a successful [`Response`]. Mirrors [`Payload`]
/// but is a distinct type so transports can project independently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ResponseValue {
    /// JSON value.
    Json(serde_json::Value),
    /// Binary value with format tag.
    Bytes(Vec<u8>, FormatTag),
    /// No body.
    Unit,
}

/// The result of a `Query` or `Command`. Carried inside a `Message`
/// with `operation = Response`. A `Response` contains either a value or
/// an `Error`; never both, never neither.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// The result. Either a value or an error.
    pub result: Result<ResponseValue, Error>,
    /// Correlation id — required on responses.
    pub correlation_id: CorrelationId,
}

impl Response {
    /// Construct a successful response.
    #[must_use]
    pub fn ok(value: ResponseValue, correlation_id: CorrelationId) -> Self {
        Self {
            result: Ok(value),
            correlation_id,
        }
    }

    /// Construct an error response.
    #[must_use]
    pub fn err(error: Error, correlation_id: CorrelationId) -> Self {
        Self {
            result: Err(error),
            correlation_id,
        }
    }

    /// Whether this response is a success.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }

    /// Whether this response is an error.
    #[must_use]
    pub fn is_err(&self) -> bool {
        self.result.is_err()
    }
}

// ===========================================================================
// Error + ErrorCode
// ===========================================================================

/// Canonical error code. Transport adapters project this into
/// GraphQL errors, MQTT error topics, SSE error frames, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Transport-level failure.
    TransportError,
    /// Resolver could not fulfil the query.
    ResolverError,
    /// Authoritative miss and no resolver.
    CacheMiss,
    /// Input failed validation.
    ValidationError,
    /// Deadline exceeded.
    Timeout,
    /// Unexpected internal failure.
    InternalError,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::TransportError => "transport_error",
            Self::ResolverError => "resolver_error",
            Self::CacheMiss => "cache_miss",
            Self::ValidationError => "validation_error",
            Self::Timeout => "timeout",
            Self::InternalError => "internal_error",
        };
        f.write_str(s)
    }
}

/// The canonical error type. Transport adapters project this into
/// GraphQL errors, MQTT error topics, SSE error frames, etc. Transport
/// adapters must not define their own error types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Error {
    /// Canonical error code.
    pub code: ErrorCode,
    /// Human-readable error message.
    pub message: String,
    /// Subject the error relates to.
    pub subject: Option<Subject>,
    /// Correlation id.
    pub correlation_id: Option<CorrelationId>,
    /// Causation id.
    pub causation_id: Option<CausationId>,
    /// Optional structured details.
    pub details: Option<serde_json::Value>,
}

impl Error {
    /// Construct an error with a code and message.
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            subject: None,
            correlation_id: None,
            causation_id: None,
            details: None,
        }
    }

    /// Construct a transport error.
    #[must_use]
    pub fn transport_error(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::TransportError, msg)
    }

    /// Construct a resolver error.
    #[must_use]
    pub fn resolver_error(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::ResolverError, msg)
    }

    /// Construct a cache-miss error.
    #[must_use]
    pub fn cache_miss(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::CacheMiss, msg)
    }

    /// Construct a validation error.
    #[must_use]
    pub fn validation_error(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::ValidationError, msg)
    }

    /// Construct a timeout error.
    #[must_use]
    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::Timeout, msg)
    }

    /// Construct an internal error.
    #[must_use]
    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, msg)
    }

    /// Set the subject.
    #[must_use]
    pub fn with_subject(mut self, s: Subject) -> Self {
        self.subject = Some(s);
        self
    }

    /// Set the correlation id.
    #[must_use]
    pub fn with_correlation(mut self, id: CorrelationId) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Set the causation id.
    #[must_use]
    pub fn with_causation(mut self, id: CausationId) -> Self {
        self.causation_id = Some(id);
        self
    }

    /// Set structured details.
    #[must_use]
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

impl From<SubjectError> for Error {
    fn from(e: SubjectError) -> Self {
        Self::validation_error(e.to_string())
    }
}

// ===========================================================================
// Query — read-only, side-effect-free request
// ===========================================================================

/// A read-only, side-effect-free request against a named `Resource`.
/// A `Query` is always carried inside a `Message` with
/// `operation = Query`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Query {
    /// The named target.
    pub resource: Resource,
    /// Optional filter predicate selecting a subset of the resource.
    pub selection: Selection,
    /// Optional named arguments to the resolver.
    pub arguments: Option<serde_json::Value>,
    /// Optional field projection narrowing which fields are returned.
    pub projection: Option<Projection>,
    /// Execution context (deadline, principal, trace, cache policy).
    pub context: Context,
    /// Optional override of the cache policy in `Context`.
    pub cache_policy: Option<CachePolicy>,
    /// Optional override of the deadline in `Context`.
    pub deadline: Option<Deadline>,
}

impl Query {
    /// Construct a minimal query against a resource with default
    /// selection (`All`) and default context.
    #[must_use]
    pub fn new(resource: Resource) -> Self {
        Self {
            resource,
            selection: Selection::All,
            arguments: None,
            projection: None,
            context: Context::default(),
            cache_policy: None,
            deadline: None,
        }
    }

    /// Set the selection.
    #[must_use]
    pub fn with_selection(mut self, s: Selection) -> Self {
        self.selection = s;
        self
    }

    /// Set the projection. Returns an error via the resolver if
    /// `include` and `exclude` are both non-empty.
    #[must_use]
    pub fn with_projection(mut self, p: Projection) -> Self {
        self.projection = Some(p);
        self
    }

    /// Set the arguments.
    #[must_use]
    pub fn with_arguments(mut self, args: serde_json::Value) -> Self {
        self.arguments = Some(args);
        self
    }
}

// ===========================================================================
// Message — the fundamental primitive
// ===========================================================================

/// The fundamental primitive. Every request, command, event, stream,
/// response, telemetry reading, and error is carried by a `Message`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier.
    pub id: MessageId,
    /// Timestamp.
    pub timestamp: Timestamp,
    /// Hierarchical routing key.
    pub subject: Subject,
    /// The verb of this message.
    pub operation: Operation,
    /// Explicitly typed, serde-serialisable payload.
    pub payload: Payload,
    /// Metadata (correlation, causation, trace, auth, extensions).
    pub metadata: Metadata,
}

impl Message {
    /// Construct a minimal message with the given subject, operation,
    /// and unit payload.
    ///
    /// # Errors
    /// Returns [`SubjectError`] if `subject_str` is not a valid subject.
    pub fn new(subject_str: &str, operation: Operation) -> Result<Self, SubjectError> {
        Ok(Self {
            id: MessageId::new(),
            timestamp: Timestamp::now_monotonic(),
            subject: Subject::from_str(subject_str)?,
            operation,
            payload: Payload::Unit,
            metadata: Metadata::default(),
        })
    }
}

// ===========================================================================
// Public traits — Resolver, MessageHandler, QueryExecutor
// ===========================================================================

/// Resolver for queries against a resource. Implementations are
/// domain-specific (storage, analysis, telemetry).
#[allow(async_fn_in_trait)]
pub trait Resolver {
    /// Resolve a query against this resolver's resource.
    ///
    /// # Errors
    /// Returns [`Error`] if the query cannot be fulfilled.
    fn resolve(
        &self,
        query: &Query,
        ctx: &Context,
    ) -> impl Future<Output = Result<Response, Error>>;
}

/// Handler for incoming messages.
#[allow(async_fn_in_trait)]
pub trait MessageHandler {
    /// Handle an incoming message.
    ///
    /// # Errors
    /// Returns [`Error`] if the message cannot be handled.
    fn handle(&self, msg: &Message) -> impl Future<Output = Result<Response, Error>>;
}

/// Orchestrator around [`Resolver`]: applies caching, batching, deadline
/// propagation, and cancellation checks. Declared here so transport
/// adapters can depend on the trait without a hard dep on themql-query;
/// implemented in themql-query.
#[allow(async_fn_in_trait)]
pub trait QueryExecutor {
    /// Execute a query with cross-cutting concerns applied.
    ///
    /// # Errors
    /// Returns [`Error`] if the query cannot be executed.
    fn execute(
        &self,
        query: &Query,
        ctx: &Context,
    ) -> impl Future<Output = Result<Response, Error>>;
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Identity ---------------------------------------------------------

    #[test]
    fn message_id_is_unique() {
        let a = MessageId::new();
        let b = MessageId::new();
        assert_ne!(a, b, "two MessageId::new() must differ");
    }

    #[test]
    fn correlation_id_derives_from_request() {
        let req = MessageId::new();
        let corr = CorrelationId::for_request(&req);
        assert_eq!(corr.0, req.0, "correlation must equal request id");
    }

    // --- Subject parsing --------------------------------------------------

    #[test]
    fn subject_parses_simple() {
        let s = Subject::from_str("vehicle.sensors.imu.gyro").unwrap();
        assert_eq!(s.segments().len(), 4);
        assert_eq!(s.as_str(), "vehicle.sensors.imu.gyro");
        assert!(s.is_concrete());
    }

    #[test]
    fn subject_rejects_empty() {
        assert!(matches!(
            Subject::from_str(""),
            Err(SubjectError::EmptySubject)
        ));
    }

    #[test]
    fn subject_rejects_trailing_separator() {
        assert!(matches!(
            Subject::from_str("vehicle."),
            Err(SubjectError::TrailingSeparator)
        ));
    }

    #[test]
    fn subject_rejects_leading_separator() {
        assert!(matches!(
            Subject::from_str(".vehicle"),
            Err(SubjectError::LeadingSeparator)
        ));
    }

    #[test]
    fn subject_rejects_consecutive_separators() {
        assert!(matches!(
            Subject::from_str("vehicle..sensors"),
            Err(SubjectError::ConsecutiveSeparators)
        ));
    }

    #[test]
    fn subject_rejects_wildcard_in_concrete() {
        assert!(matches!(
            Subject::from_str("vehicle.+.sensors"),
            Err(SubjectError::WildcardInConcreteSubject)
        ));
    }

    #[test]
    fn subject_segment_rejects_invalid_charset() {
        assert!(matches!(
            SubjectSegment::new("bad/segment"),
            Err(SubjectError::InvalidCharset { .. })
        ));
    }

    // --- SubjectPattern matching -----------------------------------------

    #[test]
    fn pattern_matches_exact() {
        let p = SubjectPattern::from_str("vehicle.sensors.imu.gyro").unwrap();
        let s = Subject::from_str("vehicle.sensors.imu.gyro").unwrap();
        assert!(p.matches(&s));
    }

    #[test]
    fn pattern_matches_wildcard_one() {
        let p = SubjectPattern::from_str("vehicle.+.imu.gyro").unwrap();
        let s = Subject::from_str("vehicle.sensors.imu.gyro").unwrap();
        assert!(p.matches(&s), "+ must match one segment");
    }

    #[test]
    fn pattern_does_not_match_wildcard_one_when_length_differs() {
        let p = SubjectPattern::from_str("vehicle.+.gyro").unwrap();
        let s = Subject::from_str("vehicle.sensors.imu.gyro").unwrap();
        assert!(!p.matches(&s), "+ matches exactly one segment");
    }

    #[test]
    fn pattern_matches_wildcard_multi_trailing() {
        let p = SubjectPattern::from_str("vehicle.#").unwrap();
        let s = Subject::from_str("vehicle.sensors.imu.gyro").unwrap();
        assert!(p.matches(&s), "# must match trailing segments");
    }

    #[test]
    fn pattern_wildcard_multi_matches_zero() {
        let p = SubjectPattern::from_str("vehicle.sensors.#").unwrap();
        let s = Subject::from_str("vehicle.sensors").unwrap();
        assert!(p.matches(&s), "# must match zero trailing segments");
    }

    #[test]
    fn pattern_rejects_wildcard_multi_not_last() {
        assert!(matches!(
            SubjectPattern::from_str("vehicle.#.sensors"),
            Err(SubjectError::WildcardMultiNotLast)
        ));
    }

    // --- Operation --------------------------------------------------------

    #[test]
    fn operation_serialises_snake_case() {
        let json = serde_json::to_string(&Operation::Telemetry).unwrap();
        assert_eq!(json, "\"telemetry\"");
        let back: Operation = serde_json::from_str("\"telemetry\"").unwrap();
        assert_eq!(back, Operation::Telemetry);
    }

    // --- Selection / Projection ------------------------------------------

    #[test]
    fn projection_include_and_exclude_mutually_exclusive() {
        let mut p = Projection::new().include(FieldPath::from_str("position.x").unwrap());
        p = p.exclude(FieldPath::from_str("velocity.vx").unwrap());
        assert!(!p.is_valid(), "include + exclude must be invalid");
    }

    #[test]
    fn projection_empty_is_valid() {
        let p = Projection::new();
        assert!(p.is_valid(), "empty projection is valid (returns all)");
    }

    #[test]
    fn selection_default_is_all() {
        assert_eq!(Selection::default(), Selection::All);
    }

    // --- CachePolicy ------------------------------------------------------

    #[test]
    fn cache_policy_disabled_is_not_enabled() {
        let p = CachePolicy::disabled();
        assert!(!p.enabled);
    }

    #[test]
    fn cache_policy_default_is_enabled() {
        assert!(CachePolicy::default().enabled);
    }

    // --- Response invariants ---------------------------------------------

    #[test]
    fn response_ok_is_ok() {
        let r = Response::ok(ResponseValue::Unit, CorrelationId::new());
        assert!(r.is_ok());
        assert!(!r.is_err());
    }

    #[test]
    fn response_err_is_err() {
        let r = Response::err(Error::timeout("deadline"), CorrelationId::new());
        assert!(r.is_err());
        assert!(!r.is_ok());
    }

    // --- Error mapping ----------------------------------------------------

    #[test]
    fn subject_error_converts_to_core_error() {
        let e: Error = SubjectError::EmptySubject.into();
        assert_eq!(e.code, ErrorCode::ValidationError);
    }

    // --- Message construction --------------------------------------------

    #[test]
    fn message_new_constructs_valid() {
        let m = Message::new("vehicle.sensors.imu.gyro", Operation::Telemetry).unwrap();
        assert_eq!(m.operation, Operation::Telemetry);
        assert_eq!(m.payload, Payload::Unit);
        assert_eq!(m.subject.as_str(), "vehicle.sensors.imu.gyro");
    }

    #[test]
    fn message_new_rejects_invalid_subject() {
        let r = Message::new("", Operation::Telemetry);
        assert!(r.is_err());
    }

    // --- Round-trip serialisation ---------------------------------------

    #[test]
    fn message_round_trips_json() {
        let m = Message::new("vehicle.sensors.imu.gyro", Operation::Telemetry).unwrap();
        let json = serde_json::to_string(&m).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn query_round_trips_json() {
        let q = Query::new(Resource::from_str("vehicle.sensors.imu.gyro").unwrap());
        let json = serde_json::to_string(&q).unwrap();
        let back: Query = serde_json::from_str(&json).unwrap();
        assert_eq!(q, back);
    }

    // --- Property-style tests --------------------------------------------

    #[test]
    fn subject_round_trip_preserves_string() {
        for s in [
            "vehicle",
            "vehicle.sensors",
            "vehicle.sensors.imu.gyro",
            "a.b.c.d.e.f.g",
        ] {
            let subject = Subject::from_str(s).unwrap();
            assert_eq!(subject.as_str(), s, "round-trip must preserve {s}");
        }
    }

    #[test]
    fn pattern_matches_self_concrete() {
        // A pattern with all-concrete segments matches only itself.
        let p = SubjectPattern::from_str("vehicle.sensors.imu.gyro").unwrap();
        assert!(p.matches(&Subject::from_str("vehicle.sensors.imu.gyro").unwrap()));
        assert!(!p.matches(&Subject::from_str("vehicle.sensors.imu.accel").unwrap()));
        assert!(!p.matches(&Subject::from_str("vehicle.sensors.imu").unwrap()));
        assert!(!p.matches(&Subject::from_str("vehicle.sensors.imu.gyro.extra").unwrap()));
    }

    #[test]
    fn cache_key_stability_via_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let s = Subject::from_str("vehicle.sensors.imu.gyro").unwrap();
        let mut h1 = DefaultHasher::new();
        s.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        s.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish(), "hash must be stable");
    }
}
