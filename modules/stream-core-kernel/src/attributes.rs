//! Stream attributes used to annotate stages and graphs.

#[cfg(test)]
#[path = "attributes_test.rs"]
mod tests;

mod async_boundary_attr;
mod attribute;
mod cancellation_strategy_kind;
mod debug_logging;
mod dispatcher_attribute;
mod fuzzing_mode;
mod input_buffer;
mod log_level;
mod log_levels;
mod mandatory_attribute;
mod max_fixed_buffer_size;
mod name;
mod nested_materialization_cancellation_policy;
mod output_burst_limit;
mod source_location;
mod stream_ref_buffer_capacity;
mod stream_ref_demand_redelivery_interval;
mod stream_ref_final_termination_signal_deadline;
mod stream_ref_subscription_timeout;
mod stream_subscription_timeout;
mod sync_processing_limit;

pub use async_boundary_attr::AsyncBoundaryAttr;
pub use attribute::Attribute;
pub use cancellation_strategy_kind::CancellationStrategyKind;
pub use debug_logging::DebugLogging;
pub use dispatcher_attribute::DispatcherAttribute;
pub use fuzzing_mode::FuzzingMode;
pub use input_buffer::InputBuffer;
pub use log_level::LogLevel;
pub use log_levels::LogLevels;
pub use mandatory_attribute::MandatoryAttribute;
pub use max_fixed_buffer_size::MaxFixedBufferSize;
pub use name::Name;
pub use nested_materialization_cancellation_policy::NestedMaterializationCancellationPolicy;
pub use output_burst_limit::OutputBurstLimit;
pub use source_location::SourceLocation;
pub use stream_ref_buffer_capacity::StreamRefBufferCapacity;
pub use stream_ref_demand_redelivery_interval::StreamRefDemandRedeliveryInterval;
pub use stream_ref_final_termination_signal_deadline::StreamRefFinalTerminationSignalDeadline;
pub use stream_ref_subscription_timeout::StreamRefSubscriptionTimeout;
pub use stream_subscription_timeout::StreamSubscriptionTimeout;
pub use sync_processing_limit::SyncProcessingLimit;

mod collection {
  use alloc::{borrow::Cow, boxed::Box, string::String, vec::Vec};

  use super::{
    AsyncBoundaryAttr, Attribute, CancellationStrategyKind, DebugLogging, DispatcherAttribute, FuzzingMode,
    InputBuffer, LogLevel, LogLevels, MandatoryAttribute, MaxFixedBufferSize, Name,
    NestedMaterializationCancellationPolicy, OutputBurstLimit, SourceLocation, StreamRefBufferCapacity,
    StreamRefDemandRedeliveryInterval, StreamRefFinalTerminationSignalDeadline, StreamRefSubscriptionTimeout,
    StreamSubscriptionTimeout, SyncProcessingLimit,
  };
  use crate::{StreamDslError, stream_subscription_timeout_termination_mode::StreamSubscriptionTimeoutTerminationMode};

  const STREAM_REF_SUBSCRIPTION_TIMEOUT_NAME: &str = "stream-ref-subscription-timeout";
  const STREAM_REF_BUFFER_CAPACITY_NAME: &str = "stream-ref-buffer-capacity";
  const STREAM_REF_DEMAND_REDELIVERY_INTERVAL_NAME: &str = "stream-ref-demand-redelivery-interval";
  const STREAM_REF_FINAL_TERMINATION_SIGNAL_DEADLINE_NAME: &str = "stream-ref-final-termination-signal-deadline";

  /// Immutable collection of stream attributes.
  ///
  /// Supports both named string attributes (legacy) and typed
  /// [`Attribute`] trait objects with downcast-based retrieval.
  #[derive(Debug)]
  pub struct Attributes {
    names: Vec<String>,
    attrs: Vec<Box<dyn Attribute>>,
  }

  impl Attributes {
    /// Creates an empty attributes collection.
    #[must_use]
    pub const fn new() -> Self {
      Self { names: Vec::new(), attrs: Vec::new() }
    }

    /// Creates attributes containing a single stage name.
    ///
    /// Stores the name both in the legacy `names` accessor and as a typed
    /// [`Name`] attribute, mirroring Pekko's `Attributes(Name(n))`.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
      let name_string = name.into();
      Self { names: alloc::vec![name_string.clone()], attrs: alloc::vec![Box::new(Name(name_string))] }
    }

    /// Creates attributes with an [`InputBuffer`] configuration.
    #[must_use]
    pub fn input_buffer(initial: usize, max: usize) -> Self {
      Self {
        names: alloc::vec![String::from("input-buffer")],
        attrs: alloc::vec![Box::new(InputBuffer::new(initial, max))],
      }
    }

    /// Creates attributes with a [`LogLevels`] configuration.
    #[must_use]
    pub fn log_levels(on_element: LogLevel, on_finish: LogLevel, on_failure: LogLevel) -> Self {
      Self {
        names: alloc::vec![String::from("log-levels")],
        attrs: alloc::vec![Box::new(LogLevels::new(on_element, on_finish, on_failure))],
      }
    }

    /// Creates attributes containing an [`AsyncBoundaryAttr`] marker.
    ///
    /// Mirrors Pekko's `Attributes.asyncBoundary`.
    #[must_use]
    pub fn async_boundary() -> Self {
      Self { names: alloc::vec![String::from("async-boundary")], attrs: alloc::vec![Box::new(AsyncBoundaryAttr)] }
    }

    /// Creates attributes containing a [`DispatcherAttribute`].
    ///
    /// A dispatcher attribute implies an async boundary; the materializer
    /// uses the named dispatcher for the resulting island.
    #[must_use]
    pub fn dispatcher(name: impl Into<String>) -> Self {
      Self {
        names: alloc::vec![String::from("dispatcher")],
        attrs: alloc::vec![Box::new(DispatcherAttribute::new(name))],
      }
    }

    /// Creates attributes containing a [`SourceLocation`] callsite.
    ///
    /// Mirrors Pekko's `SourceLocation.forLambda`, with the JVM lambda
    /// reference replaced by the Rust-native callsite triple
    /// `(file, line, column)`.
    #[must_use]
    pub fn source_location(file: impl Into<Cow<'static, str>>, line: u32, column: u32) -> Self {
      Self {
        names: alloc::vec![String::from("source-location")],
        attrs: alloc::vec![Box::new(SourceLocation::new(file.into(), line, column))],
      }
    }

    /// Appends names and typed attributes from another collection.
    #[must_use]
    pub fn and(mut self, other: Self) -> Self {
      self.names.extend(other.names);
      self.attrs.extend(other.attrs);
      self
    }

    /// Retrieves a typed attribute by its concrete type.
    ///
    /// Returns `None` if no attribute of type `T` is stored.
    #[must_use]
    pub fn get<T: Attribute + 'static>(&self) -> Option<&T> {
      self.attrs.iter().find_map(|attr| attr.as_any().downcast_ref::<T>())
    }

    /// Returns `true` if an attribute of type `T` is stored.
    #[must_use]
    pub fn contains<T: Attribute + 'static>(&self) -> bool {
      self.get::<T>().is_some()
    }

    /// Returns all stored attributes of type `T`.
    #[must_use]
    pub fn get_all<T: Attribute + 'static>(&self) -> Vec<&T> {
      self.attrs.iter().filter_map(|attr| attr.as_any().downcast_ref::<T>()).collect()
    }

    /// Retrieves a mandatory typed attribute by its concrete type.
    ///
    /// Restricts `T` to [`MandatoryAttribute`] implementers, mirroring
    /// Pekko's `mandatoryAttribute[T <: MandatoryAttribute]` at compile
    /// time instead of the runtime hierarchy check.
    #[must_use]
    pub fn mandatory_attribute<T: MandatoryAttribute + 'static>(&self) -> Option<&T> {
      self.get::<T>()
    }

    /// Creates attributes containing a [`CancellationStrategyKind`].
    #[must_use]
    pub fn cancellation_strategy(strategy: CancellationStrategyKind) -> Self {
      Self { names: alloc::vec![String::from("cancellation-strategy")], attrs: alloc::vec![Box::new(strategy)] }
    }

    /// Creates attributes containing a [`NestedMaterializationCancellationPolicy`].
    ///
    /// Mirrors Pekko's `Attributes(NestedMaterializationCancellationPolicy(...))`
    /// factory helper.
    #[must_use]
    pub fn nested_materialization_cancellation_policy(policy: NestedMaterializationCancellationPolicy) -> Self {
      Self {
        names: alloc::vec![String::from("nested-materialization-cancellation-policy")],
        attrs: alloc::vec![Box::new(policy)],
      }
    }

    /// Creates attributes containing a [`DebugLogging`] flag.
    ///
    /// Mirrors Pekko's `Attributes(DebugLogging(enabled))`.
    #[must_use]
    pub fn debug_logging(enabled: bool) -> Self {
      Self {
        names: alloc::vec![String::from("debug-logging")],
        attrs: alloc::vec![Box::new(DebugLogging::new(enabled))],
      }
    }

    /// Creates attributes containing a [`FuzzingMode`] flag.
    ///
    /// Mirrors Pekko's `Attributes(FuzzingMode(enabled))`.
    #[must_use]
    pub fn fuzzing_mode(enabled: bool) -> Self {
      Self { names: alloc::vec![String::from("fuzzing-mode")], attrs: alloc::vec![Box::new(FuzzingMode::new(enabled))] }
    }

    /// Creates attributes containing a [`MaxFixedBufferSize`].
    ///
    /// Mirrors Pekko's `Attributes(MaxFixedBufferSize(size))`.
    #[must_use]
    pub fn max_fixed_buffer_size(size: usize) -> Self {
      Self {
        names: alloc::vec![String::from("max-fixed-buffer-size")],
        attrs: alloc::vec![Box::new(MaxFixedBufferSize::new(size))],
      }
    }

    /// Creates attributes containing an [`OutputBurstLimit`].
    ///
    /// Mirrors Pekko's `Attributes(OutputBurstLimit(limit))`.
    #[must_use]
    pub fn output_burst_limit(limit: usize) -> Self {
      Self {
        names: alloc::vec![String::from("output-burst-limit")],
        attrs: alloc::vec![Box::new(OutputBurstLimit::new(limit))],
      }
    }

    /// Creates attributes containing a [`StreamSubscriptionTimeout`].
    ///
    /// Mirrors Pekko's `Attributes(StreamSubscriptionTimeout(timeout, mode))`.
    /// The `timeout_ticks` value is expressed in scheduler ticks (no_std).
    #[must_use]
    pub fn stream_subscription_timeout(
      timeout_ticks: u32,
      termination_mode: StreamSubscriptionTimeoutTerminationMode,
    ) -> Self {
      Self {
        names: alloc::vec![String::from("stream-subscription-timeout")],
        attrs: alloc::vec![Box::new(StreamSubscriptionTimeout::new(timeout_ticks, termination_mode))],
      }
    }

    /// Creates attributes containing a [`StreamRefSubscriptionTimeout`].
    ///
    /// Mirrors Pekko's `StreamRefAttributes.subscriptionTimeout`.
    #[must_use]
    pub fn stream_ref_subscription_timeout(timeout_ticks: u32) -> Self {
      Self {
        names: alloc::vec![String::from(STREAM_REF_SUBSCRIPTION_TIMEOUT_NAME)],
        attrs: alloc::vec![Box::new(StreamRefSubscriptionTimeout::new(timeout_ticks))],
      }
    }

    /// Creates attributes containing a [`StreamRefBufferCapacity`].
    ///
    /// Mirrors Pekko's `StreamRefAttributes.bufferCapacity`.
    ///
    /// # Errors
    ///
    /// Returns [`StreamDslError::InvalidArgument`] when `capacity == 0`.
    pub fn stream_ref_buffer_capacity(capacity: usize) -> Result<Self, StreamDslError> {
      let capacity = StreamRefBufferCapacity::new(capacity)?;
      Ok(Self {
        names: alloc::vec![String::from(STREAM_REF_BUFFER_CAPACITY_NAME)],
        attrs: alloc::vec![Box::new(capacity)],
      })
    }

    /// Creates attributes containing a [`StreamRefDemandRedeliveryInterval`].
    ///
    /// Mirrors Pekko's `StreamRefAttributes.demandRedeliveryInterval`.
    #[must_use]
    pub fn stream_ref_demand_redelivery_interval(timeout_ticks: u32) -> Self {
      Self {
        names: alloc::vec![String::from(STREAM_REF_DEMAND_REDELIVERY_INTERVAL_NAME)],
        attrs: alloc::vec![Box::new(StreamRefDemandRedeliveryInterval::new(timeout_ticks))],
      }
    }

    /// Creates attributes containing a [`StreamRefFinalTerminationSignalDeadline`].
    ///
    /// Mirrors Pekko's `StreamRefAttributes.finalTerminationSignalDeadline`.
    #[must_use]
    pub fn stream_ref_final_termination_signal_deadline(timeout_ticks: u32) -> Self {
      Self {
        names: alloc::vec![String::from(STREAM_REF_FINAL_TERMINATION_SIGNAL_DEADLINE_NAME)],
        attrs: alloc::vec![Box::new(StreamRefFinalTerminationSignalDeadline::new(timeout_ticks))],
      }
    }

    /// Creates attributes containing a [`SyncProcessingLimit`].
    ///
    /// Mirrors Pekko's `Attributes(SyncProcessingLimit(limit))`.
    #[must_use]
    pub fn sync_processing_limit(limit: usize) -> Self {
      Self {
        names: alloc::vec![String::from("sync-processing-limit")],
        attrs: alloc::vec![Box::new(SyncProcessingLimit::new(limit))],
      }
    }

    /// Returns all configured stage names.
    #[must_use]
    pub fn names(&self) -> &[String] {
      &self.names
    }

    /// Returns `true` when no attributes have been configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
      self.names.is_empty() && self.attrs.is_empty()
    }

    /// Returns `true` when these attributes indicate an async boundary.
    ///
    /// An async boundary is indicated by either an [`AsyncBoundaryAttr`]
    /// or a [`DispatcherAttribute`] (which implies an async boundary).
    /// This mirrors Pekko's `Attributes.isAsync` logic.
    #[must_use]
    pub fn is_async(&self) -> bool {
      self.get::<AsyncBoundaryAttr>().is_some() || self.get::<DispatcherAttribute>().is_some()
    }
  }

  impl Default for Attributes {
    fn default() -> Self {
      Self::new()
    }
  }

  impl Clone for Attributes {
    fn clone(&self) -> Self {
      Self { names: self.names.clone(), attrs: self.attrs.iter().map(|attr| attr.clone_box()).collect() }
    }
  }

  impl PartialEq for Attributes {
    fn eq(&self, other: &Self) -> bool {
      self.names == other.names
        && self.attrs.len() == other.attrs.len()
        && self.attrs.iter().zip(other.attrs.iter()).all(|(a, b)| a.eq_attr(b.as_any()))
    }
  }

  impl Eq for Attributes {}
}

pub use collection::Attributes;
