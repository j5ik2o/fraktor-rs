use alloc::boxed::Box;
use core::{any::TypeId, marker::PhantomData};

use super::{
  BoundedSourceQueue, DynValue, MatCombine, OverflowStrategy, SourceDefinition, SourceLogic, StageDefinition,
  StageKind, StreamError, StreamGraph, SupervisionStrategy, shape::Outlet, source::Source,
};
use crate::{attributes::Attributes, r#impl::queue::ActorSourceRef};

#[cfg(test)]
mod tests;

/// Actor-oriented source factory utilities.
///
/// Provides factory methods to create sources whose materialized value
/// is an [`ActorSourceRef`] — a handle that external code can use to
/// push elements into the stream.
pub struct ActorSource;

const ACK_BACKPRESSURE_BUFFER_SIZE: usize = 16;

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
    let definition = SourceDefinition {
      kind:        StageKind::Custom,
      outlet:      Outlet::<T>::next_id(),
      output_type: TypeId::of::<T>(),
      mat_combine: MatCombine::Right,
      supervision: SupervisionStrategy::Stop,
      restart:     None,
      logic:       Box::new(logic),
      attributes:  Attributes::new(),
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
    // ack ベースプロトコルの内部バッファ。実際のバックプレッシャーは ack
    // ハンドシェイクで制御されるため、キューオーバーフローではなく ack 待ちで
    // 流量が調整される。バッファサイズ 16 は ack 応答前に複数の tell を
    // キューイングするための余裕であり、Pekko の実装に倣った値。
    let queue = BoundedSourceQueue::new(ACK_BACKPRESSURE_BUFFER_SIZE, OverflowStrategy::Fail);
    let source_ref = ActorSourceRef::new(queue.clone());
    let logic = ActorRefBackpressureSourceLogic::<T, Ack, ReceiveAck> {
      queue,
      ack_message,
      receive_ack,
      awaiting_ack: false,
      _pd: PhantomData,
    };
    let mut graph = StreamGraph::new();
    let definition = SourceDefinition {
      kind:        StageKind::Custom,
      outlet:      Outlet::<T>::next_id(),
      output_type: TypeId::of::<T>(),
      mat_combine: MatCombine::Right,
      supervision: SupervisionStrategy::Stop,
      restart:     None,
      logic:       Box::new(logic),
      attributes:  Attributes::new(),
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

  fn on_shutdown(&mut self) -> Result<(), StreamError> {
    if self.queue.complete_if_open() || self.queue.is_closed() { Ok(()) } else { Err(StreamError::Failed) }
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

  fn on_shutdown(&mut self) -> Result<(), StreamError> {
    if self.queue.complete_if_open() || self.queue.is_closed() { Ok(()) } else { Err(StreamError::Failed) }
  }
}
