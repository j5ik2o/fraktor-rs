//! Errors produced when attempting to send messages to a mailbox or actor.

use alloc::string::String;
use core::fmt;

use crate::pid::Pid;

/// Failures produced while attempting to send a message.
#[derive(Debug)]
pub enum SendError<T> {
  /// The destination could not be resolved (e.g. dangling reference or stopped actor).
  NoRecipient {
    /// Identifier of the target when known.
    pid:     Option<Pid>,
    /// Message that failed to send.
    message: T,
  },
  /// The mailbox rejected the message because it is saturated and configured to apply backpressure.
  MailboxFull {
    /// Identifier of the target when known.
    pid:     Option<Pid>,
    /// Message that failed to send.
    message: T,
  },
  /// The mailbox is currently suspended and cannot accept user messages.
  MailboxSuspended {
    /// Identifier of the target when known.
    pid:     Option<Pid>,
    /// Message that failed to send.
    message: T,
  },
  /// The receiver has already completed an ask-style future, so additional replies are discarded.
  AlreadyResponded {
    /// Message that failed to send.
    message: T,
    /// Optional reason captured for diagnostics.
    detail:  Option<String>,
  },
  /// Generic failure for unsupported or shutdown states.
  Closed {
    /// Identifier of the target when known.
    pid:     Option<Pid>,
    /// Message that failed to send.
    message: T,
  },
}

impl<T> SendError<T> {
  /// Constructs a new `NoRecipient` error.
  #[must_use]
  pub fn no_recipient(pid: Option<Pid>, message: T) -> Self {
    Self::NoRecipient { pid, message }
  }

  /// Constructs a new `MailboxFull` error.
  #[must_use]
  pub fn mailbox_full(pid: Option<Pid>, message: T) -> Self {
    Self::MailboxFull { pid, message }
  }

  /// Constructs a new `MailboxSuspended` error.
  #[must_use]
  pub fn mailbox_suspended(pid: Option<Pid>, message: T) -> Self {
    Self::MailboxSuspended { pid, message }
  }

  /// Constructs a new `Closed` error.
  #[must_use]
  pub fn closed(pid: Option<Pid>, message: T) -> Self {
    Self::Closed { pid, message }
  }

  /// Constructs a new `AlreadyResponded` error with optional diagnostic detail.
  #[must_use]
  pub fn already_responded(message: T, detail: Option<String>) -> Self {
    Self::AlreadyResponded { message, detail }
  }

  /// Borrows the failed message.
  #[must_use]
  pub fn message(&self) -> &T {
    match self {
      | Self::NoRecipient { message, .. }
      | Self::MailboxFull { message, .. }
      | Self::MailboxSuspended { message, .. }
      | Self::AlreadyResponded { message, .. }
      | Self::Closed { message, .. } => message,
    }
  }

  /// Consumes the error and returns the owned message.
  #[must_use]
  pub fn into_message(self) -> T {
    match self {
      | Self::NoRecipient { message, .. }
      | Self::MailboxFull { message, .. }
      | Self::MailboxSuspended { message, .. }
      | Self::AlreadyResponded { message, .. }
      | Self::Closed { message, .. } => message,
    }
  }

  /// Returns the PID associated with the failure when available.
  #[must_use]
  pub const fn pid(&self) -> Option<Pid> {
    match self {
      | Self::NoRecipient { pid, .. }
      | Self::MailboxFull { pid, .. }
      | Self::MailboxSuspended { pid, .. }
      | Self::Closed { pid, .. } => *pid,
      | Self::AlreadyResponded { .. } => None,
    }
  }
}

impl<T> fmt::Display for SendError<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::NoRecipient { pid, .. } => write!(f, "no recipient available for pid {:?}", pid),
      | Self::MailboxFull { pid, .. } => write!(f, "mailbox full for pid {:?}", pid),
      | Self::MailboxSuspended { pid, .. } => write!(f, "mailbox suspended for pid {:?}", pid),
      | Self::AlreadyResponded { detail, .. } => {
        if let Some(detail) = detail {
          write!(f, "ask already responded ({detail})")
        } else {
          write!(f, "ask already responded")
        }
      },
      | Self::Closed { pid, .. } => write!(f, "mailbox closed for pid {:?}", pid),
    }
  }
}
