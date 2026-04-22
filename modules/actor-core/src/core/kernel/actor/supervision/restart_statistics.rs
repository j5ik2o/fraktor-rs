//! Tracks restart attempts for supervised actors using Pekko's one-shot
//! window algorithm.
//!
//! Pekko reference:
//! `references/pekko/actor/src/main/scala/org/apache/pekko/actor/FaultHandling.scala:48-86`
//! (`ChildRestartStats.requestRestartPermission` and
//! `retriesInWindowOkay`).
//!
//! The internal state mirrors Pekko's `maxNrOfRetriesCount: Int` and
//! `restartTimeWindowStartNanos: Long` pair. The original sliding-window
//! implementation (`Vec<Duration>` of failure timestamps pruned on each
//! record) has been removed: Pekko's contract is a one-shot window that
//! resets counter and window-start in a single step when the window
//! expires, and sliding-window semantics produced observable divergence
//! from Pekko on concurrent restart timelines.
//!
//! `now: Duration` across this module is a **monotonic** clock reading
//! (matches Pekko's `System.nanoTime()` — see `FaultHandling.scala:71`).
//! fraktor-rs callers obtain it via `ActorSystem::monotonic_now()`.
//! Passing a wall-clock value risks window breakage on system-clock
//! adjustments and is not supported.

use core::time::Duration;

use super::restart_limit::RestartLimit;

/// Pekko-parity restart statistics holder.
///
/// Internal state tracks:
/// - `restart_count`: restarts observed within the current window
/// - `window_start`: start time of the current window, or `None` when no window is active (before
///   the first permit, or just after a reset)
///
/// `Duration::ZERO` passed as the `window` argument to
/// [`RestartStatistics::request_restart_permission`] is the fraktor-rs
/// sentinel for "no window" (typed Pekko `withinTimeRange = Duration.Zero`
/// default, classic Pekko `withinTimeRangeOption` returning `None` — both
/// agree that `Duration.Zero` means window-less). It is **not** a 0 ms
/// window.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RestartStatistics {
  restart_count: u32,
  window_start:  Option<Duration>,
}

impl RestartStatistics {
  /// Creates an empty statistics container (no prior restarts, no window).
  #[must_use]
  pub const fn new() -> Self {
    Self { restart_count: 0, window_start: None }
  }

  /// Returns the current in-window restart count.
  #[must_use]
  pub const fn restart_count(&self) -> u32 {
    self.restart_count
  }

  /// Returns the current window start, or `None` when no window is active.
  #[must_use]
  pub const fn window_start(&self) -> Option<Duration> {
    self.window_start
  }

  /// Pekko `ChildRestartStats.requestRestartPermission` direct port
  /// (`FaultHandling.scala:56-62`). Returns `true` if the caller should
  /// restart, `false` if it should stop.
  ///
  /// CQS note: this method intentionally combines state mutation
  /// (`restart_count` / `window_start`) with a boolean return. The
  /// `.agents/rules/rust/cqs-principle.md` treats this as an allowed
  /// exception modelled on Pekko's own design — splitting the check and
  /// the apply would introduce a TOCTOU gap. See `design.md` Decision 3
  /// of the `pekko-supervision-max-restarts-semantic` change for the
  /// rationale.
  pub fn request_restart_permission(&mut self, now: Duration, limit: RestartLimit, window: Duration) -> bool {
    match (limit, window.is_zero()) {
      // Pekko `(Some(0), _) if retries < 1 => false`. Counter is left
      // untouched so no reset-on-false side effect is necessary for
      // correctness; `handle_failure` still calls `reset()` to mirror
      // Pekko's child-death effect.
      | (RestartLimit::WithinWindow(0), _) => false,
      // Pekko `(None, _) => true` — unlimited with no window.
      | (RestartLimit::Unlimited, true) => true,
      // Pekko `(Some(n), None) => count += 1; count <= n`.
      | (RestartLimit::WithinWindow(n), true) => {
        self.restart_count = self.restart_count.saturating_add(1);
        self.restart_count <= n
      },
      // Pekko `(None, Some(window)) => retriesInWindowOkay(1, window)`.
      // The hard-coded `retries = 1` reproduces Pekko's quirk where an
      // "Unlimited" strategy combined with a finite window denies the
      // second in-window failure.
      | (RestartLimit::Unlimited, false) => self.retries_in_window_okay(1, window, now),
      // Pekko `(Some(n), Some(window)) => retriesInWindowOkay(n, window)`.
      | (RestartLimit::WithinWindow(n), false) => self.retries_in_window_okay(n, window, now),
    }
  }

  /// Clears restart count and window-start (called by `handle_failure`
  /// after a `Stop` / `Escalate` / permission-denied outcome).
  pub const fn reset(&mut self) {
    self.restart_count = 0;
    self.window_start = None;
  }

  /// Pekko `retriesInWindowOkay` direct port (`FaultHandling.scala:64-86`).
  ///
  /// Lines marked with `// Pekko:` indicate the corresponding statement in
  /// the Scala source.
  fn retries_in_window_okay(&mut self, retries: u32, window: Duration, now: Duration) -> bool {
    // Pekko: val retriesDone = maxNrOfRetriesCount + 1
    let retries_done = self.restart_count.saturating_add(1);
    // Pekko: val windowStart = if (restartTimeWindowStartNanos == 0) { ... now } else
    // restartTimeWindowStartNanos
    let window_start = match self.window_start {
      | Some(ws) => ws,
      | None => {
        self.window_start = Some(now);
        now
      },
    };
    // Pekko: val insideWindow = (now - windowStart) <= TimeUnit.MILLISECONDS.toNanos(window)
    let inside_window = now.saturating_sub(window_start) <= window;
    if inside_window {
      // Pekko: maxNrOfRetriesCount = retriesDone; retriesDone <= retries
      self.restart_count = retries_done;
      retries_done <= retries
    } else {
      // Pekko: maxNrOfRetriesCount = 1; restartTimeWindowStartNanos = now; true
      self.restart_count = 1;
      self.window_start = Some(now);
      true
    }
  }
}
