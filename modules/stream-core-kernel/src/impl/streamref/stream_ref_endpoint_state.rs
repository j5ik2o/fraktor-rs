#[cfg(test)]
#[path = "stream_ref_endpoint_state_test.rs"]
mod tests;

use alloc::borrow::Cow;

use crate::StreamError;

const DUPLICATE_MATERIALIZATION_MESSAGE: &str = "stream ref was materialized more than once";
const INVALID_PARTNER_MESSAGE: &str = "stream ref message came from a non-partner actor";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamRefEndpointTerminal {
  Completed,
  Cancelled,
  Failed,
}

/// State shared by actor-backed StreamRef endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StreamRefEndpointState {
  partner_ref:           Option<Cow<'static, str>>,
  terminal:              Option<StreamRefEndpointTerminal>,
  failure:               Option<StreamError>,
  watch_release_failure: Option<StreamError>,
  shutdown_failure:      Option<StreamError>,
  shutdown_requested:    bool,
}

impl StreamRefEndpointState {
  /// Creates an unpaired endpoint state.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self {
      partner_ref:           None,
      terminal:              None,
      failure:               None,
      watch_release_failure: None,
      shutdown_failure:      None,
      shutdown_requested:    false,
    }
  }

  /// Records the first StreamRef partner.
  pub(crate) fn pair_partner(&mut self, got_ref: impl Into<Cow<'static, str>>) -> Result<(), StreamError> {
    let got_ref = got_ref.into();
    if let Some(expected_ref) = &self.partner_ref {
      return Err(Self::invalid_partner(
        expected_ref.clone(),
        got_ref,
        Cow::Borrowed(DUPLICATE_MATERIALIZATION_MESSAGE),
      ));
    }
    self.partner_ref = Some(got_ref);
    Ok(())
  }

  /// Verifies that a protocol message came from the paired partner.
  pub(crate) fn ensure_partner(&self, got_ref: impl Into<Cow<'static, str>>) -> Result<(), StreamError> {
    let got_ref = got_ref.into();
    let Some(expected_ref) = &self.partner_ref else {
      return Err(StreamError::StreamRefTargetNotInitialized);
    };
    if expected_ref.as_ref() == got_ref.as_ref() {
      return Ok(());
    }
    Err(Self::invalid_partner(expected_ref.clone(), got_ref, Cow::Borrowed(INVALID_PARTNER_MESSAGE)))
  }

  /// Records normal completion and requests endpoint shutdown.
  pub(crate) const fn complete(&mut self) {
    self.transition_terminal(StreamRefEndpointTerminal::Completed);
  }

  /// Records cancellation and requests endpoint shutdown.
  pub(crate) const fn cancel(&mut self) {
    self.transition_terminal(StreamRefEndpointTerminal::Cancelled);
  }

  /// Records failure and requests endpoint shutdown.
  pub(crate) fn fail(&mut self, error: StreamError) {
    if self.terminal.is_some() {
      return;
    }
    self.failure = Some(error);
    self.transition_terminal(StreamRefEndpointTerminal::Failed);
  }

  /// Records a failed partner watch release.
  pub(crate) fn record_watch_release_failure(&mut self, error: StreamError) {
    if self.watch_release_failure.is_none() {
      self.watch_release_failure = Some(error);
    }
  }

  /// Records a failed endpoint shutdown.
  pub(crate) fn record_shutdown_failure(&mut self, error: StreamError) {
    if self.shutdown_failure.is_none() {
      self.shutdown_failure = Some(error);
    }
  }

  /// Returns the paired partner reference.
  #[must_use]
  pub(crate) fn partner_ref(&self) -> Option<&str> {
    self.partner_ref.as_deref()
  }

  /// Returns `true` when the endpoint has reached normal completion.
  #[must_use]
  pub(crate) const fn is_completed(&self) -> bool {
    matches!(self.terminal, Some(StreamRefEndpointTerminal::Completed))
  }

  /// Returns `true` when the endpoint has reached cancellation.
  #[must_use]
  pub(crate) const fn is_cancelled(&self) -> bool {
    matches!(self.terminal, Some(StreamRefEndpointTerminal::Cancelled))
  }

  /// Returns `true` when the endpoint has reached failure.
  #[must_use]
  pub(crate) const fn is_failed(&self) -> bool {
    matches!(self.terminal, Some(StreamRefEndpointTerminal::Failed))
  }

  /// Returns the recorded stream failure.
  #[must_use]
  pub(crate) const fn failure(&self) -> Option<&StreamError> {
    self.failure.as_ref()
  }

  /// Returns a watch release failure recorded during endpoint cleanup.
  #[must_use]
  pub(crate) const fn watch_release_failure(&self) -> Option<&StreamError> {
    self.watch_release_failure.as_ref()
  }

  /// Returns a shutdown failure recorded during endpoint cleanup.
  #[must_use]
  pub(crate) const fn shutdown_failure(&self) -> Option<&StreamError> {
    self.shutdown_failure.as_ref()
  }

  /// Returns `true` when the endpoint actor must be stopped.
  #[must_use]
  pub(crate) const fn is_shutdown_requested(&self) -> bool {
    self.shutdown_requested
  }

  const fn transition_terminal(&mut self, terminal: StreamRefEndpointTerminal) {
    if self.terminal.is_none() {
      self.terminal = Some(terminal);
      self.shutdown_requested = true;
    }
  }

  const fn invalid_partner(
    expected_ref: Cow<'static, str>,
    got_ref: Cow<'static, str>,
    message: Cow<'static, str>,
  ) -> StreamError {
    StreamError::InvalidPartnerActor { expected_ref, got_ref, message }
  }
}
