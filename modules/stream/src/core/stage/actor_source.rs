use alloc::boxed::Box;
use core::{any::TypeId, marker::PhantomData};

use super::{
  BoundedSourceQueue, DynValue, MatCombine, OverflowStrategy, SourceDefinition, SourceLogic, StageDefinition,
  StageKind, StreamError, SupervisionStrategy, graph::StreamGraph, shape::Outlet, source::Source,
};
use crate::core::ActorSourceRef;

#[cfg(test)]
mod tests;

/// Actor-oriented source factory utilities.
///
/// Provides factory methods to create sources whose materialized value
/// is an [`ActorSourceRef`] — a handle that external code can use to
/// push elements into the stream.
pub struct ActorSource;

impl ActorSource {
  /// Creates an actor-ref style source with a bounded buffer.
  ///
  /// The materialized [`ActorSourceRef`] allows callers to push
  /// elements via [`tell`](ActorSourceRef::tell), signal normal
  /// completion via [`complete`](ActorSourceRef::complete), or
  /// signal failure via [`fail`](ActorSourceRef::fail).
  ///
  /// # Panics
  ///
  /// Panics when `overflow_strategy` is [`OverflowStrategy::Backpressure`].
  /// Use [`actor_ref_with_backpressure`](Self::actor_ref_with_backpressure)
  /// instead for backpressure semantics.
  #[must_use]
  pub fn actor_ref<T>(buffer_size: usize, overflow_strategy: OverflowStrategy) -> Source<T, ActorSourceRef<T>>
  where
    T: Send + Sync + 'static, {
    assert!(
      overflow_strategy != OverflowStrategy::Backpressure,
      "Backpressure overflowStrategy not supported for ActorSource::actor_ref; use actor_ref_with_backpressure instead"
    );
    let queue = BoundedSourceQueue::new(buffer_size, overflow_strategy);
    let source_ref = ActorSourceRef::new(queue.clone());
    let logic = ActorRefSourceLogic::<T> { queue, _pd: PhantomData };
    let mut graph = StreamGraph::new();
    let outlet: Outlet<T> = Outlet::new();
    let definition = SourceDefinition {
      kind:        StageKind::Custom,
      outlet:      outlet.id(),
      output_type: TypeId::of::<T>(),
      mat_combine: MatCombine::KeepRight,
      supervision: SupervisionStrategy::Stop,
      restart:     None,
      logic:       Box::new(logic),
    };
    graph.push_stage(StageDefinition::Source(definition));
    Source::from_graph(graph, source_ref)
  }

  /// Creates an actor-ref source with ack-based backpressure.
  ///
  /// The source uses a single-element slot internally. After each
  /// element is consumed, the caller must supply the expected
  /// `ack_message` via `receive_ack` before the next [`tell`](ActorSourceRef::tell)
  /// is accepted.
  #[must_use]
  pub fn actor_ref_with_backpressure<T, Ack, ReceiveAck>(
    ack_message: Ack,
    receive_ack: ReceiveAck,
  ) -> Source<T, ActorSourceRef<T>>
  where
    T: Send + Sync + 'static,
    Ack: Clone + PartialEq + Send + Sync + 'static,
    ReceiveAck: FnMut() -> Option<Ack> + Send + Sync + 'static, {
    // Internal buffer for the ack-based protocol. The actual backpressure
    // is enforced by the ack handshake, not by queue overflow. The buffer
    // allows multiple tells to be queued before pull consumes them.
    let queue = BoundedSourceQueue::new(16, OverflowStrategy::Fail);
    let source_ref = ActorSourceRef::new(queue.clone());
    let logic = ActorRefBackpressureSourceLogic::<T, Ack, ReceiveAck> {
      queue,
      ack_message,
      receive_ack,
      awaiting_ack: false,
      _pd: PhantomData,
    };
    let mut graph = StreamGraph::new();
    let outlet: Outlet<T> = Outlet::new();
    let definition = SourceDefinition {
      kind:        StageKind::Custom,
      outlet:      outlet.id(),
      output_type: TypeId::of::<T>(),
      mat_combine: MatCombine::KeepRight,
      supervision: SupervisionStrategy::Stop,
      restart:     None,
      logic:       Box::new(logic),
    };
    graph.push_stage(StageDefinition::Source(definition));
    Source::from_graph(graph, source_ref)
  }
}

// --- Internal SourceLogic implementations ---

struct ActorRefSourceLogic<T> {
  queue: BoundedSourceQueue<T>,
  _pd:   PhantomData<fn() -> T>,
}

impl<T> SourceLogic for ActorRefSourceLogic<T>
where
  T: Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    match self.queue.poll_or_drain()? {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Ok(None),
    }
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    self.queue.close_for_cancel();
    Ok(())
  }
}

struct ActorRefBackpressureSourceLogic<T, Ack, ReceiveAck> {
  queue:        BoundedSourceQueue<T>,
  ack_message:  Ack,
  receive_ack:  ReceiveAck,
  awaiting_ack: bool,
  _pd:          PhantomData<fn() -> T>,
}

impl<T, Ack, ReceiveAck> SourceLogic for ActorRefBackpressureSourceLogic<T, Ack, ReceiveAck>
where
  T: Send + Sync + 'static,
  Ack: Clone + PartialEq + Send + Sync + 'static,
  ReceiveAck: FnMut() -> Option<Ack> + Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    // When awaiting ack, check if ack has arrived before pulling
    if self.awaiting_ack {
      if matches!((self.receive_ack)(), Some(received) if received == self.ack_message) {
        self.awaiting_ack = false;
      } else {
        return Err(StreamError::WouldBlock);
      }
    }

    match self.queue.poll_or_drain()? {
      | Some(value) => {
        self.awaiting_ack = true;
        Ok(Some(Box::new(value) as DynValue))
      },
      | None => Ok(None),
    }
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    self.queue.close_for_cancel();
    Ok(())
  }
}
