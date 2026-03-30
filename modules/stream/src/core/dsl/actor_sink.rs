use core::marker::PhantomData;

use super::{
  DemandTracker, DynValue, SinkDecision, SinkLogic, StageKind, StreamCompletion, StreamDone, StreamError,
  downcast_value, sink::Sink,
};

#[cfg(test)]
mod tests;

/// Actor-oriented sink factory utilities.
pub struct ActorSink;

impl ActorSink {
  /// Creates an actor-ref style sink.
  pub fn actor_ref<In, Emit>(emit: Emit) -> Sink<In, StreamCompletion<StreamDone>>
  where
    In: Send + Sync + 'static,
    Emit: FnMut(In) + Send + Sync + 'static, {
    let completion = StreamCompletion::new();
    let logic = ActorRefSinkLogic { emit, completion: completion.clone(), _pd: PhantomData };
    Sink::from_definition(StageKind::Custom, logic, completion)
  }

  /// Creates an actor-ref style sink whose callback can fail.
  pub fn actor_ref_with_result<In, Emit>(emit: Emit) -> Sink<In, StreamCompletion<StreamDone>>
  where
    In: Send + Sync + 'static,
    Emit: FnMut(In) -> Result<(), StreamError> + Send + Sync + 'static, {
    let completion = StreamCompletion::new();
    let logic = ActorRefResultSinkLogic { emit, completion: completion.clone(), _pd: PhantomData };
    Sink::from_definition(StageKind::Custom, logic, completion)
  }

  /// Creates an actor-ref sink with backpressure semantics.
  ///
  /// The sink sends the `on_init_message` first, waits for `receive_ack` to yield
  /// `ack_message`, and then forwards each stream element using `message_adapter`.
  /// After each sent message, demand is requested only when `receive_ack` yields
  /// the expected acknowledgement.
  pub fn actor_ref_with_backpressure<
    In,
    Ack,
    Msg,
    Emit,
    MessageAdapter,
    OnInitMessage,
    ReceiveAck,
    OnFailureMessage,
  >(
    emit: Emit,
    message_adapter: MessageAdapter,
    on_init_message: OnInitMessage,
    receive_ack: ReceiveAck,
    ack_message: Ack,
    on_complete_message: Msg,
    on_failure_message: OnFailureMessage,
  ) -> Sink<In, StreamCompletion<StreamDone>>
  where
    In: Send + Sync + 'static,
    Ack: Clone + PartialEq + Send + Sync + 'static,
    Msg: Clone + Send + Sync + 'static,
    Emit: FnMut(Msg) + Send + Sync + 'static,
    MessageAdapter: FnMut(Ack, In) -> Msg + Send + Sync + 'static,
    OnInitMessage: FnMut(Ack) -> Msg + Send + Sync + 'static,
    ReceiveAck: FnMut() -> Option<Ack> + Send + Sync + 'static,
    OnFailureMessage: FnMut(StreamError) -> Msg + Send + Sync + 'static, {
    let completion = StreamCompletion::new();
    let logic = ActorRefBackpressureSinkLogic::<
      In,
      Ack,
      Msg,
      Emit,
      MessageAdapter,
      OnInitMessage,
      ReceiveAck,
      OnFailureMessage,
    > {
      emit,
      message_adapter,
      on_init_message,
      receive_ack,
      ack_message,
      on_complete_message,
      on_failure_message,
      completion: completion.clone(),
      awaiting_ack: false,
      _pd: PhantomData,
    };
    Sink::from_definition(StageKind::Custom, logic, completion)
  }
}

struct ActorRefSinkLogic<In, Emit> {
  emit:       Emit,
  completion: StreamCompletion<StreamDone>,
  _pd:        PhantomData<fn(In)>,
}

struct ActorRefResultSinkLogic<In, Emit> {
  emit:       Emit,
  completion: StreamCompletion<StreamDone>,
  _pd:        PhantomData<fn(In)>,
}

impl<In, Emit> SinkLogic for ActorRefSinkLogic<In, Emit>
where
  In: Send + Sync + 'static,
  Emit: FnMut(In) + Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    (self.emit)(downcast_value::<In>(input)?);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.completion.complete(Ok(StreamDone::new()));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

impl<In, Emit> SinkLogic for ActorRefResultSinkLogic<In, Emit>
where
  In: Send + Sync + 'static,
  Emit: FnMut(In) -> Result<(), StreamError> + Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    (self.emit)(downcast_value::<In>(input)?)?;
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.completion.complete(Ok(StreamDone::new()));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

struct ActorRefBackpressureSinkLogic<In, Ack, Msg, Emit, MessageAdapter, OnInitMessage, ReceiveAck, OnFailureMessage> {
  emit:                Emit,
  message_adapter:     MessageAdapter,
  on_init_message:     OnInitMessage,
  receive_ack:         ReceiveAck,
  ack_message:         Ack,
  on_complete_message: Msg,
  on_failure_message:  OnFailureMessage,
  completion:          StreamCompletion<StreamDone>,
  awaiting_ack:        bool,
  _pd:                 PhantomData<fn(In, Msg)>,
}

impl<In, Ack, Msg, Emit, MessageAdapter, OnInitMessage, ReceiveAck, OnFailureMessage> SinkLogic
  for ActorRefBackpressureSinkLogic<In, Ack, Msg, Emit, MessageAdapter, OnInitMessage, ReceiveAck, OnFailureMessage>
where
  In: Send + Sync + 'static,
  Ack: Clone + PartialEq + Send + Sync + 'static,
  Msg: Clone + Send + Sync + 'static,
  Emit: FnMut(Msg) + Send + Sync + 'static,
  MessageAdapter: FnMut(Ack, In) -> Msg + Send + Sync + 'static,
  OnInitMessage: FnMut(Ack) -> Msg + Send + Sync + 'static,
  ReceiveAck: FnMut() -> Option<Ack> + Send + Sync + 'static,
  OnFailureMessage: FnMut(StreamError) -> Msg + Send + Sync + 'static,
{
  fn can_accept_input(&self) -> bool {
    !self.awaiting_ack
  }

  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    (self.emit)((self.on_init_message)(self.ack_message.clone()));
    self.awaiting_ack = true;
    self.observe_ack(demand).map(|_| ())
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    if self.awaiting_ack {
      return Err(StreamError::WouldBlock);
    }
    let value = downcast_value::<In>(input)?;
    (self.emit)((self.message_adapter)(self.ack_message.clone(), value));
    self.awaiting_ack = true;
    let _ = self.observe_ack(demand)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    (self.emit)(self.on_complete_message.clone());
    self.completion.complete(Ok(StreamDone::new()));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    (self.emit)((self.on_failure_message)(error.clone()));
    self.completion.complete(Err(error));
  }

  fn on_tick(&mut self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    self.observe_ack(demand)
  }
}

impl<In, Ack, Msg, Emit, MessageAdapter, OnInitMessage, ReceiveAck, OnFailureMessage>
  ActorRefBackpressureSinkLogic<In, Ack, Msg, Emit, MessageAdapter, OnInitMessage, ReceiveAck, OnFailureMessage>
where
  In: Send + Sync + 'static,
  Ack: Clone + PartialEq + Send + Sync + 'static,
  Msg: Clone + Send + Sync + 'static,
  Emit: FnMut(Msg) + Send + Sync + 'static,
  MessageAdapter: FnMut(Ack, In) -> Msg + Send + Sync + 'static,
  OnInitMessage: FnMut(Ack) -> Msg + Send + Sync + 'static,
  ReceiveAck: FnMut() -> Option<Ack> + Send + Sync + 'static,
  OnFailureMessage: FnMut(StreamError) -> Msg + Send + Sync + 'static,
{
  fn observe_ack(&mut self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    if !self.awaiting_ack {
      return Ok(false);
    }
    if matches!((self.receive_ack)(), Some(received) if received == self.ack_message) {
      self.awaiting_ack = false;
      demand.request(1)?;
      return Ok(true);
    }
    Ok(false)
  }
}
