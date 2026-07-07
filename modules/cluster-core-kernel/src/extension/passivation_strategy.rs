//! Passivation strategy settings for cluster sharding.

use core::time::Duration;

use super::ClusterShardingSettingsError;

#[cfg(test)]
#[path = "passivation_strategy_test.rs"]
mod tests;

/// Passivation strategy for sharded entities.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum PassivationStrategy {
  /// Passivation is disabled.
  #[default]
  Disabled,
  /// Passivate entities after an idle timeout.
  Idle {
    /// Idle timeout after which an entity is passivated.
    timeout:        Duration,
    /// Optional interval between idle checks; defaults to half of `timeout` when unset at runtime.
    check_interval: Option<Duration>,
  },
  /// Passivate when the active entity count exceeds a limit.
  ActiveLimit {
    /// Maximum number of concurrently active entities.
    limit:          u32,
    /// Optional idle timeout applied in addition to the active limit.
    idle_timeout:   Option<Duration>,
    /// Optional idle check interval used with `idle_timeout`.
    check_interval: Option<Duration>,
  },
  /// Passivate using a least-recently-used replacement policy.
  Lru {
    /// Maximum number of concurrently active entities.
    limit:                 u32,
    /// Optional segmented LRU proportions; empty means a single segment.
    segmented_proportions: alloc::vec::Vec<f64>,
    /// Optional idle timeout applied in addition to the replacement policy.
    idle_timeout:          Option<Duration>,
    /// Optional idle check interval used with `idle_timeout`.
    check_interval:        Option<Duration>,
  },
  /// Passivate using a most-recently-used replacement policy.
  Mru {
    /// Maximum number of concurrently active entities.
    limit:          u32,
    /// Optional idle timeout applied in addition to the replacement policy.
    idle_timeout:   Option<Duration>,
    /// Optional idle check interval used with `idle_timeout`.
    check_interval: Option<Duration>,
  },
  /// Passivate using a least-frequently-used replacement policy.
  Lfu {
    /// Maximum number of concurrently active entities.
    limit:          u32,
    /// Whether dynamic aging is enabled for the frequency counter.
    dynamic_aging:  bool,
    /// Optional idle timeout applied in addition to the replacement policy.
    idle_timeout:   Option<Duration>,
    /// Optional idle check interval used with `idle_timeout`.
    check_interval: Option<Duration>,
  },
}

impl PassivationStrategy {
  /// Returns whether passivation is disabled.
  #[must_use]
  pub const fn is_disabled(&self) -> bool {
    matches!(self, Self::Disabled)
  }

  /// Validates passivation strategy settings.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterShardingSettingsError`] when a limit or idle timeout is zero.
  pub fn validate(&self) -> Result<(), ClusterShardingSettingsError> {
    match self {
      | Self::Disabled => Ok(()),
      | Self::Idle { timeout, .. } => {
        if *timeout == Duration::ZERO {
          return Err(ClusterShardingSettingsError::ZeroIdleTimeout);
        }
        Ok(())
      },
      | Self::ActiveLimit { limit, idle_timeout, .. } => {
        Self::validate_limit(*limit)?;
        Self::validate_optional_idle(*idle_timeout)?;
        Ok(())
      },
      | Self::Lru { limit, idle_timeout, .. } => {
        Self::validate_limit(*limit)?;
        Self::validate_optional_idle(*idle_timeout)?;
        Ok(())
      },
      | Self::Mru { limit, idle_timeout, .. } => {
        Self::validate_limit(*limit)?;
        Self::validate_optional_idle(*idle_timeout)?;
        Ok(())
      },
      | Self::Lfu { limit, idle_timeout, .. } => {
        Self::validate_limit(*limit)?;
        Self::validate_optional_idle(*idle_timeout)?;
        Ok(())
      },
    }
  }

  const fn validate_limit(limit: u32) -> Result<(), ClusterShardingSettingsError> {
    if limit == 0 {
      return Err(ClusterShardingSettingsError::ZeroActiveEntityLimit);
    }
    Ok(())
  }

  fn validate_optional_idle(idle_timeout: Option<Duration>) -> Result<(), ClusterShardingSettingsError> {
    if idle_timeout == Some(Duration::ZERO) {
      return Err(ClusterShardingSettingsError::ZeroIdleTimeout);
    }
    Ok(())
  }
}
