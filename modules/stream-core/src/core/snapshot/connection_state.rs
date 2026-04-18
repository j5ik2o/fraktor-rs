#[cfg(test)]
mod tests;

/// Runtime state of a connection between two logic nodes.
///
/// Corresponds to Pekko `pekko.stream.impl.fusing.GraphInterpreter.ConnectionState`
/// (a sealed trait with three object instances). Modelled here as a plain
/// `Copy` enum because the variants carry no payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionState {
  /// The downstream must pull the upstream for more elements.
  ShouldPull,
  /// The upstream has an element ready to push to the downstream.
  ShouldPush,
  /// The connection has been closed and will produce no further elements.
  Closed,
}
