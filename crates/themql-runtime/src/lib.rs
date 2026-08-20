//! # themql-runtime
//!
//! Execution runtime abstraction for theMQL. Desktop uses tokio; embedded
//! uses embassy. Library crates depend on one of two runtime traits —
//! [`DesktopRuntime`] or [`EmbeddedRuntime`] — selected at compile time via
//! `cfg(feature = "desktop")` / `cfg(feature = "embedded")`.
//!
//! There is NO shared `Runtime` trait: tokio and embassy differ enough in
//! shape that a unifying trait would force speculative abstractions
//! (forbidden by `prompts/IMPLEMENTER.md`). This is a deliberate user
//! decision (2026-08-19); see `specs/runtime.toml`.
//!
//! ## Feature flags
//!
//! - `desktop` — enables the [`DesktopRuntime`] trait and the
//!   [`TokioRuntime`] implementation. Pulls in `tokio`.
//! - `embedded` — enables the [`EmbeddedRuntime`] trait and the
//!   [`EmbassyRuntime`] implementation. Pulls in `embassy-sync` and
//!   `embassy-executor`.
//!
//! Neither feature is enabled by default; a downstream crate enables
//! exactly the one matching its target domain.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::fmt;

#[cfg(any(feature = "desktop", feature = "embedded"))]
use std::future::Future;

// ===========================================================================
// Error types — always available, regardless of feature flags
// ===========================================================================

/// Error returned when a spawned task's join handle completes with a
/// failure (e.g. the task panicked or was cancelled). Wraps a backend
/// error string; the desktop `tokio` implementation converts
/// `tokio::task::JoinError` into this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinError {
    reason: String,
}

impl JoinError {
    /// Construct a `JoinError` from a textual reason.
    #[must_use]
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    /// The textual reason the join failed.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("join error: ")?;
        f.write_str(&self.reason)
    }
}

impl std::error::Error for JoinError {}

/// Error returned when a timeout expires before the wrapped future
/// completes. Mirrors `tokio::time::error::Elapsed` but is a distinct
/// type owned by this crate so the trait is backend-agnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elapsed;

impl fmt::Display for Elapsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("operation timed out")
    }
}

impl std::error::Error for Elapsed {}

/// Canonical runtime error type. Combines join failures, timeouts, and
/// backend-shape mismatches into a single enum used by both runtime
/// traits.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeError {
    /// A spawned task's join handle completed with a failure.
    #[error("join error: {0}")]
    Join(#[from] JoinError),
    /// A timeout expired before the wrapped future completed.
    #[error("timeout elapsed")]
    Elapsed(#[from] Elapsed),
    /// The runtime backend rejected an operation for shape reasons
    /// (e.g. spawning on a shut-down executor).
    #[error("backend error: {0}")]
    Backend(String),
}

// ===========================================================================
// DesktopRuntime — tokio-backed (cfg feature = "desktop")
// ===========================================================================

/// Sender half of a bounded channel produced by [`DesktopRuntime::channel`].
///
/// This is a thin newtype wrapper around `tokio::sync::mpsc::Sender` so the
/// trait signature does not name a backend type directly. Constructed only
/// by [`TokioRuntime`]; library crates receive it from
/// [`DesktopRuntime::channel`].
#[cfg(feature = "desktop")]
#[derive(Debug)]
pub struct Sender<T> {
    tx: tokio::sync::mpsc::Sender<T>,
}

#[cfg(feature = "desktop")]
impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

#[cfg(feature = "desktop")]
impl<T> Sender<T> {
    /// Send a value, awaiting backpressure.
    ///
    /// # Errors
    /// Returns [`RuntimeError::Backend`] if the receiver has been dropped.
    pub async fn send(&self, value: T) -> Result<(), RuntimeError> {
        self.tx
            .send(value)
            .await
            .map_err(|_| RuntimeError::Backend("receiver dropped".to_owned()))
    }
}

/// Receiver half of a bounded channel produced by
/// [`DesktopRuntime::channel`].
#[cfg(feature = "desktop")]
#[derive(Debug)]
pub struct Receiver<T> {
    rx: tokio::sync::mpsc::Receiver<T>,
}

#[cfg(feature = "desktop")]
impl<T> Receiver<T> {
    /// Receive the next value, or `None` if all senders have been dropped.
    pub async fn recv(&mut self) -> Option<T> {
        self.rx.recv().await
    }
}

/// Cancellation token produced by [`DesktopRuntime::cancellation`].
///
/// Thin wrapper around `tokio_util::sync::CancellationToken`-shaped
/// semantics; here it is backed by `tokio::sync::Notify` to keep the
/// dependency surface to `tokio` only. Cloning shares the underlying
/// signal.
#[cfg(feature = "desktop")]
#[derive(Debug, Clone)]
pub struct CancellationToken {
    notify: std::sync::Arc<tokio::sync::Notify>,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(feature = "desktop")]
impl CancellationToken {
    fn new() -> Self {
        Self {
            notify: std::sync::Arc::new(tokio::sync::Notify::new()),
            cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Mark this token cancelled and wake all waiters.
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
        self.notify.notify_waiters();
    }

    /// Whether this token has been cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Resolves when the token is cancelled. If already cancelled, resolves
    /// immediately.
    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        loop {
            let notified = self.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
            if self.is_cancelled() {
                return;
            }
        }
    }
}

/// Tokio-backed runtime trait. Implemented once in `themql-runtime` behind
/// `cfg(feature = "desktop")`. Library crates with domain `desktop` or
/// `cross-cutting` depend on this trait.
///
/// The trait is a thin re-export of tokio primitives; we do NOT
/// reimplement tokio. It exists so library crates depend on a stable name
/// and binary crates can substitute a mock in tests.
#[cfg(feature = "desktop")]
#[allow(async_fn_in_trait)]
pub trait DesktopRuntime: Send + Sync + 'static {
    /// Spawn a future on the runtime; returns a join handle that resolves
    /// to the future's output or a [`JoinError`] on failure.
    fn spawn<F, T>(&self, fut: F) -> impl Future<Output = Result<T, JoinError>> + Send + Unpin
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static;

    /// Non-blocking delay.
    fn sleep(&self, dur: std::time::Duration) -> impl Future<Output = ()> + Send;

    /// Wrap a future with a deadline; returns `Err(Elapsed)` on expiry.
    fn timeout<F, T>(
        &self,
        dur: std::time::Duration,
        fut: F,
    ) -> impl Future<Output = Result<T, Elapsed>> + Send
    where
        F: Future<Output = T> + Send,
        T: Send;

    /// Bounded mpsc channel with explicit backpressure.
    fn channel<T: Send>(&self, bound: usize) -> (Sender<T>, Receiver<T>);

    /// Returns a [`CancellationToken`] that propagates through async paths.
    fn cancellation(&self) -> CancellationToken;
}

/// Tokio-backed implementation of [`DesktopRuntime`].
///
/// Holds a handle to a `tokio` runtime; constructed by the desktop binary
/// and passed to library crates that depend on the trait.
#[cfg(feature = "desktop")]
#[derive(Debug, Clone)]
pub struct TokioRuntime {
    handle: tokio::runtime::Handle,
}

#[cfg(feature = "desktop")]
impl TokioRuntime {
    /// Construct a `TokioRuntime` from an existing tokio runtime handle.
    #[must_use]
    pub fn new(handle: tokio::runtime::Handle) -> Self {
        Self { handle }
    }

    /// The underlying tokio handle.
    #[must_use]
    pub fn handle(&self) -> &tokio::runtime::Handle {
        &self.handle
    }
}

#[cfg(feature = "desktop")]
impl DesktopRuntime for TokioRuntime {
    fn spawn<F, T>(&self, fut: F) -> impl Future<Output = Result<T, JoinError>> + Send + Unpin
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let join = self.handle.spawn(fut);
        Box::pin(async move { join.await.map_err(|e| JoinError::new(e.to_string())) })
    }

    fn sleep(&self, dur: std::time::Duration) -> impl Future<Output = ()> + Send {
        tokio::time::sleep(dur)
    }

    fn timeout<F, T>(
        &self,
        dur: std::time::Duration,
        fut: F,
    ) -> impl Future<Output = Result<T, Elapsed>> + Send
    where
        F: Future<Output = T> + Send,
        T: Send,
    {
        Box::pin(async move { tokio::time::timeout(dur, fut).await.map_err(|_| Elapsed) })
    }

    fn channel<T: Send>(&self, bound: usize) -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = tokio::sync::mpsc::channel(bound);
        (Sender { tx }, Receiver { rx })
    }

    fn cancellation(&self) -> CancellationToken {
        CancellationToken::new()
    }
}

// ===========================================================================
// EmbeddedRuntime — embassy-backed (cfg feature = "embedded")
// ===========================================================================

/// Marker value carried by an embassy cancellation signal. See
/// [`EmbeddedRuntime::cancellation`].
#[cfg(feature = "embedded")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

#[cfg(feature = "embedded")]
impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cancelled")
    }
}

/// Embassy-backed runtime trait. Implemented once in `themql-runtime`
/// behind `cfg(feature = "embedded")`. Library crates with domain
/// `embedded` or `cross-cutting-on-embedded` depend on this trait.
///
/// Embassy is async/executor-based but `no_std`-shaped; there is no
/// `JoinHandle` with a return value the way tokio has. Tasks communicate
/// results via channels/signals. The trait reflects that shape, not
/// tokio's.
#[cfg(feature = "embedded")]
pub trait EmbeddedRuntime: 'static {
    /// Spawner type returned by [`EmbeddedRuntime::spawner`]. Must be
    /// `embassy_executor::Spawner`-shaped.
    type SpawnHandle;

    /// Returns an embassy `Spawner` for spawning tasks.
    fn spawner(&self) -> Self::SpawnHandle;

    /// `embassy::time::Timer`-based delay.
    fn sleep(&self, dur: std::time::Duration) -> impl Future<Output = ()> + Send;

    /// `embassy_sync::signal::Signal` for one-shot values.
    fn signal<T: Send>(
        &self,
    ) -> embassy_sync::signal::Signal<embassy_sync::blocking_mutex::raw::NoopRawMutex, T>;

    /// Embassy `Signal<Cancelled>` checked in async loops.
    fn cancellation(
        &self,
    ) -> embassy_sync::signal::Signal<embassy_sync::blocking_mutex::raw::NoopRawMutex, Cancelled>;
}

/// Embassy-backed implementation of [`EmbeddedRuntime`].
///
/// Holds an embassy `Spawner`; constructed by the embedded binary and
/// passed to library crates that depend on the trait.
#[cfg(feature = "embedded")]
pub struct EmbassyRuntime {
    spawner: embassy_executor::Spawner,
}

#[cfg(feature = "embedded")]
impl EmbassyRuntime {
    /// Construct an `EmbassyRuntime` from an embassy spawner.
    #[must_use]
    pub fn new(spawner: embassy_executor::Spawner) -> Self {
        Self { spawner }
    }
}

#[cfg(feature = "embedded")]
impl EmbeddedRuntime for EmbassyRuntime {
    type SpawnHandle = embassy_executor::Spawner;

    fn spawner(&self) -> Self::SpawnHandle {
        self.spawner
    }

    fn sleep(&self, _dur: std::time::Duration) -> impl Future<Output = ()> + Send {
        embassy_future_sleep()
    }

    fn signal<T: Send>(
        &self,
    ) -> embassy_sync::signal::Signal<embassy_sync::blocking_mutex::raw::NoopRawMutex, T> {
        embassy_sync::signal::Signal::new()
    }

    fn cancellation(
        &self,
    ) -> embassy_sync::signal::Signal<embassy_sync::blocking_mutex::raw::NoopRawMutex, Cancelled>
    {
        embassy_sync::signal::Signal::new()
    }
}

#[cfg(feature = "embedded")]
fn embassy_future_sleep() -> impl Future<Output = ()> + Send {
    std::future::pending::<()>()
}

// ===========================================================================
// Tests — only exercise what compiles without features enabled
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_error_constructs_and_displays() {
        let e = JoinError::new("panic");
        assert_eq!(e.reason(), "panic");
        assert_eq!(e.to_string(), "join error: panic");
    }

    #[test]
    fn elapsed_displays() {
        assert_eq!(Elapsed.to_string(), "operation timed out");
    }

    #[test]
    fn runtime_error_from_join_error() {
        let je = JoinError::new("panic");
        let re: RuntimeError = je.into();
        assert_eq!(re, RuntimeError::Join(JoinError::new("panic")));
    }

    #[test]
    fn runtime_error_from_elapsed() {
        let re: RuntimeError = Elapsed.into();
        assert_eq!(re, RuntimeError::Elapsed(Elapsed));
    }

    #[test]
    fn runtime_error_backend_variant() {
        let re = RuntimeError::Backend("shut down".to_owned());
        assert_eq!(re.to_string(), "backend error: shut down");
    }

    #[test]
    fn join_error_is_std_error() {
        fn assert_std_error<E: std::error::Error>() {}
        assert_std_error::<JoinError>();
        assert_std_error::<Elapsed>();
        assert_std_error::<RuntimeError>();
    }

    // -----------------------------------------------------------------------
    // TokioRuntime real-impl tests (feature = "desktop")
    // -----------------------------------------------------------------------

    #[cfg(feature = "desktop")]
    mod tokio_tests {
        use super::*;
        use std::time::Duration;
        use tokio::runtime::Runtime;

        fn rt() -> (Runtime, TokioRuntime) {
            let runtime = Runtime::new().expect("tokio runtime");
            let handle = runtime.handle().clone();
            (runtime, TokioRuntime::new(handle))
        }

        #[test]
        fn spawn_returns_value() {
            let (_guard, runtime) = rt();
            runtime.handle().block_on(async {
                let join = runtime.spawn(async { 7_u32 + 5_u32 });
                let v = join.await.expect("spawn join");
                assert_eq!(v, 12);
            });
        }

        #[test]
        fn sleep_waits_at_least_requested_duration() {
            let (_guard, runtime) = rt();
            runtime.handle().block_on(async {
                let start = tokio::time::Instant::now();
                runtime.sleep(Duration::from_millis(20)).await;
                assert!(start.elapsed() >= Duration::from_millis(20));
            });
        }

        #[test]
        fn timeout_fires_when_future_is_too_slow() {
            let (_guard, runtime) = rt();
            runtime.handle().block_on(async {
                let slow = runtime.sleep(Duration::from_mins(1));
                let res = runtime.timeout(Duration::from_millis(10), slow).await;
                assert_eq!(res, Err(Elapsed));
            });
        }

        #[test]
        fn timeout_succeeds_when_future_completes_in_time() {
            let (_guard, runtime) = rt();
            runtime.handle().block_on(async {
                let fast = async { 42_u32 };
                let res = runtime.timeout(Duration::from_millis(50), fast).await;
                assert_eq!(res, Ok(42));
            });
        }

        #[test]
        fn channel_send_and_recv_roundtrip() {
            let (_guard, runtime) = rt();
            runtime.handle().block_on(async {
                let (tx, mut rx) = runtime.channel::<u32>(4);
                tx.send(123).await.expect("send");
                let v = rx.recv().await.expect("recv");
                assert_eq!(v, 123);
            });
        }

        #[test]
        fn cancellation_token_cancel_then_observed() {
            let (_guard, runtime) = rt();
            runtime.handle().block_on(async {
                let token = runtime.cancellation();
                assert!(!token.is_cancelled());
                token.cancel();
                assert!(token.is_cancelled());
                token.cancelled().await;
            });
        }

        #[test]
        fn cancellation_token_cancelled_resolves_if_already_cancelled() {
            let (_guard, runtime) = rt();
            runtime.handle().block_on(async {
                let token = runtime.cancellation();
                token.cancel();
                tokio::time::timeout(Duration::from_millis(50), token.cancelled())
                    .await
                    .expect("cancelled resolves immediately");
            });
        }

        #[test]
        fn spawn_join_error_when_task_panics() {
            let (_guard, runtime) = rt();
            runtime.handle().block_on(async {
                let join = runtime.spawn(async {
                    panic!("boom");
                });
                let res = join.await;
                assert!(res.is_err());
                assert!(res.unwrap_err().reason().contains("boom"));
            });
        }
    }
}
