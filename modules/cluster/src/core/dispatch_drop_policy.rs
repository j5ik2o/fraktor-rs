//! Drop policy for queued RPC requests.

/// How to behave when the per-key queue is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchDropPolicy {
  /// Drop the oldest queued request to make room for new one.
  DropOldest,
  /// Reject the new request.
  RejectNew,
}

#[cfg(test)]
mod tests;
