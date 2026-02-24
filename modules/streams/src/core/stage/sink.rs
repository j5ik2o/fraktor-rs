use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::{any::TypeId, future::Future, marker::PhantomData};

use super::{
  DynValue, MatCombine, RestartBackoff, RestartSettings, SinkDecision, SinkDefinition, SinkLogic, StageDefinition,
  StageKind, StreamCompletion, StreamDone, StreamDslError, StreamError, StreamGraph, StreamNotUsed, StreamStage,
  SupervisionStrategy, downcast_value,
  graph::{GraphStage, GraphStageLogic},
  shape::{Inlet, Outlet, StreamShape},
  source::Source,
  stage_context::StageContext,
  validate_positive_argument,
};

#[cfg(test)]
mod tests;

/// Sink stage definition.
pub struct Sink<In, Mat> {
  graph: StreamGraph,
  mat:   Mat,
  _pd:   PhantomData<fn(In)>,
}

impl<In> Sink<In, StreamCompletion<StreamDone>>
where
  In: Send + Sync + 'static,
{
  /// Creates a sink that ignores elements.
  #[must_use]
  pub fn ignore() -> Self {
    let completion = StreamCompletion::new();
    let logic = IgnoreSinkLogic::<In> { completion: completion.clone(), _pd: PhantomData };
    Self::from_definition(StageKind::SinkIgnore, logic, completion)
  }

  /// Creates a sink that applies a closure for each element.
  #[must_use]
  pub fn foreach<F>(func: F) -> Self
  where
    F: FnMut(In) + Send + Sync + 'static, {
    let completion = StreamCompletion::new();
    let logic = ForeachSinkLogic::<In, F> { func, completion: completion.clone(), _pd: PhantomData };
    Self::from_definition(StageKind::SinkForeach, logic, completion)
  }

  /// Creates a sink that cancels after receiving the first element.
  #[must_use]
  pub fn cancelled() -> Self {
    let completion = StreamCompletion::new();
    let logic = CancelledSinkLogic { completion: completion.clone() };
    Self::from_definition(StageKind::Custom, logic, completion)
  }

  /// Creates a sink that never keeps elements.
  #[must_use]
  pub fn none() -> Self {
    Self::cancelled()
  }

  /// Creates a sink that invokes callback on completion or failure.
  #[must_use]
  pub fn on_complete<F>(callback: F) -> Self
  where
    F: FnMut(Result<StreamDone, StreamError>) + Send + Sync + 'static, {
    let completion = StreamCompletion::new();
    let logic = OnCompleteSinkLogic::<In, F> { callback, completion: completion.clone(), _pd: PhantomData };
    Self::from_definition(StageKind::Custom, logic, completion)
  }

  /// Creates a sink that is compatible with completion-stage based APIs.
  #[must_use]
  pub fn completion_stage_sink() -> Self {
    Self::ignore()
  }

  /// Creates a sink that folds while a predicate remains true.
  #[must_use]
  pub fn fold_while<Acc, P, F>(initial: Acc, mut predicate: P, mut func: F) -> Sink<In, StreamCompletion<Acc>>
  where
    Acc: Send + Sync + 'static,
    P: FnMut(&Acc, &In) -> bool + Send + Sync + 'static,
    F: FnMut(Acc, In) -> Acc + Send + Sync + 'static, {
    Sink::fold(initial, move |acc, value| {
      if predicate(&acc, &value) {
        return func(acc, value);
      }
      acc
    })
  }

  /// Creates a sink that is compatible with async-foreach entry points.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn foreach_async<F, Fut>(parallelism: usize, mut func: F) -> Result<Self, StreamDslError>
  where
    F: FnMut(In) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    let _ = validate_positive_argument("parallelism", parallelism)?;
    Ok(Self::foreach(move |value| {
      core::mem::drop((func)(value));
    }))
  }

  /// Creates a sink backed by a materializer integration placeholder.
  #[must_use]
  pub fn from_materializer() -> Self {
    Self::ignore()
  }

  /// Creates a sink backed by a subscriber integration placeholder.
  #[must_use]
  pub fn from_subscriber() -> Self {
    Self::ignore()
  }

  /// Creates a sink compatible with future-based entry points.
  #[must_use]
  pub fn future_sink() -> Self {
    Self::ignore()
  }

  /// Lazily creates a completion-stage sink.
  ///
  /// Alias of [`Sink::lazy_sink`].
  #[must_use]
  pub fn lazy_completion_stage_sink<F>(factory: F) -> Self
  where
    F: FnOnce() -> Self + Send + 'static, {
    Self::lazy_sink(factory)
  }

  /// Lazily creates a future sink.
  ///
  /// Alias of [`Sink::lazy_sink`].
  #[must_use]
  pub fn lazy_future_sink<F>(factory: F) -> Self
  where
    F: FnOnce() -> Self + Send + 'static, {
    Self::lazy_sink(factory)
  }

  /// Lazily creates a sink.
  ///
  /// The factory is not called until the first element arrives.
  /// The sink logic from the created sink is extracted and used for processing.
  #[must_use]
  pub fn lazy_sink<F>(factory: F) -> Self
  where
    F: FnOnce() -> Self + Send + 'static, {
    let completion = StreamCompletion::new();
    let logic = LazySinkLogic::<In, F> {
      factory:    Some(factory),
      inner:      None,
      completion: completion.clone(),
      _pd:        PhantomData,
    };
    Self::from_definition(StageKind::Custom, logic, completion)
  }

  /// Converts this sink into a pre-materialized form.
  #[must_use]
  pub fn pre_materialize(self) -> (Self, StreamCompletion<StreamDone>) {
    (self, StreamCompletion::new())
  }

  /// Creates a source placeholder corresponding to sink-source bridge APIs.
  #[must_use]
  pub fn source() -> Source<In, StreamNotUsed> {
    Source::empty()
  }

  /// Creates a source placeholder corresponding to publisher conversion APIs.
  #[must_use]
  pub fn as_publisher(self) -> Source<In, StreamNotUsed> {
    let _ = self;
    Source::empty()
  }
}

impl<In> Sink<In, StreamCompletion<Vec<In>>>
where
  In: Send + Sync + 'static,
{
  /// Creates a sink that collects all elements into a vector.
  #[must_use]
  pub fn collect() -> Self {
    Self::fold(Vec::new(), |mut acc: Vec<In>, value| {
      acc.push(value);
      acc
    })
  }

  /// Creates a sink that collects all elements into a collection.
  #[must_use]
  pub fn collection() -> Self {
    Self::collect()
  }

  /// Creates a sink that collects all elements in sequence.
  #[must_use]
  pub fn seq() -> Self {
    Self::collect()
  }

  /// Creates a sink compatible with Java collector integration.
  #[must_use]
  pub fn java_collector() -> Self {
    Self::collect()
  }

  /// Creates a sink compatible with parallel unordered Java collector integration.
  #[must_use]
  pub fn java_collector_parallel_unordered() -> Self {
    Self::collect()
  }

  /// Creates a sink that stores only the last `limit` elements.
  #[must_use]
  pub fn take_last(limit: usize) -> Self {
    let completion = StreamCompletion::new();
    let logic = TakeLastSinkLogic::<In> {
      limit,
      values: VecDeque::with_capacity(limit),
      completion: completion.clone(),
      _pd: PhantomData,
    };
    Self::from_definition(StageKind::Custom, logic, completion)
  }
}

impl Sink<u8, StreamCompletion<Vec<u8>>> {
  /// Creates a sink that collects bytes for path-compatible output.
  #[must_use]
  pub fn to_path(path: &str) -> Self {
    let _ = path;
    Self::collect()
  }
}

impl<In> Sink<In, StreamCompletion<usize>>
where
  In: Send + Sync + 'static,
{
  /// Creates a sink that counts consumed elements.
  #[must_use]
  pub fn count() -> Self {
    Self::fold(0_usize, |acc, _| acc.saturating_add(1))
  }
}

impl<In> Sink<In, StreamCompletion<bool>>
where
  In: Send + Sync + 'static,
{
  /// Creates a sink that checks whether any element matches the predicate.
  #[must_use]
  pub fn exists<F>(mut predicate: F) -> Self
  where
    F: FnMut(&In) -> bool + Send + Sync + 'static, {
    Self::fold(false, move |acc, value| acc || predicate(&value))
  }

  /// Creates a sink that checks whether all elements match the predicate.
  #[must_use]
  pub fn forall<F>(mut predicate: F) -> Self
  where
    F: FnMut(&In) -> bool + Send + Sync + 'static, {
    Self::fold(true, move |acc, value| acc && predicate(&value))
  }
}

impl<In> Sink<In, StreamCompletion<Option<In>>>
where
  In: Send + Sync + 'static,
{
  /// Creates a sink that completes with the first element if available.
  #[must_use]
  pub fn head_option() -> Self {
    let completion = StreamCompletion::new();
    let logic =
      HeadOptionSinkLogic::<In> { completion: completion.clone(), seen: false, _pd: PhantomData };
    Self::from_definition(StageKind::Custom, logic, completion)
  }

  /// Creates a sink that completes with the last element if available.
  #[must_use]
  pub fn last_option() -> Self {
    let completion = StreamCompletion::new();
    let logic = LastOptionSinkLogic::<In> { last: None, completion: completion.clone(), _pd: PhantomData };
    Self::from_definition(StageKind::Custom, logic, completion)
  }
}

impl<In> Sink<In, StreamNotUsed>
where
  In: Send + Sync + 'static,
{
  pub(in crate::core) fn from_logic<L>(kind: StageKind, logic: L) -> Self
  where
    L: SinkLogic + 'static, {
    Self::from_definition(kind, logic, StreamNotUsed::new())
  }
}

impl<In, Acc> Sink<In, StreamCompletion<Acc>>
where
  In: Send + Sync + 'static,
  Acc: Send + Sync + 'static,
{
  /// Creates a sink that folds elements.
  #[must_use]
  pub fn fold<F>(initial: Acc, func: F) -> Self
  where
    F: FnMut(Acc, In) -> Acc + Send + Sync + 'static, {
    let completion = StreamCompletion::new();
    let logic =
      FoldSinkLogic::<In, Acc, F> { acc: Some(initial), func, completion: completion.clone(), _pd: PhantomData };
    Self::from_definition(StageKind::SinkFold, logic, completion)
  }
}

impl<In> Sink<In, StreamCompletion<In>>
where
  In: Send + Sync + 'static,
{
  /// Creates a sink that completes with the first element.
  #[must_use]
  pub fn head() -> Self {
    let completion = StreamCompletion::new();
    let logic = HeadSinkLogic::<In> { completion: completion.clone(), seen: false, _pd: PhantomData };
    Self::from_definition(StageKind::SinkHead, logic, completion)
  }

  /// Creates a sink that completes with the last element.
  #[must_use]
  pub fn last() -> Self {
    let completion = StreamCompletion::new();
    let logic = LastSinkLogic::<In> { last: None, completion: completion.clone(), _pd: PhantomData };
    Self::from_definition(StageKind::SinkLast, logic, completion)
  }

  /// Creates a sink that reduces elements by using the first element as seed.
  #[must_use]
  pub fn reduce<F>(func: F) -> Self
  where
    F: FnMut(In, In) -> In + Send + Sync + 'static, {
    let completion = StreamCompletion::new();
    let logic = ReduceSinkLogic::<In, F> { acc: None, func, completion: completion.clone(), _pd: PhantomData };
    Self::from_definition(StageKind::Custom, logic, completion)
  }
}

impl<In, Mat> Sink<In, Mat>
where
  In: Send + Sync + 'static,
{
  /// Creates a sink from a pre-built stream graph and materialized value.
  #[must_use]
  pub fn from_graph(graph: StreamGraph, mat: Mat) -> Self {
    Self { graph, mat, _pd: PhantomData }
  }

  pub(crate) fn into_parts(self) -> (StreamGraph, Mat) {
    (self.graph, self.mat)
  }

  /// Maps this sink materialized value.
  #[must_use]
  pub fn map_materialized_value<Mat2, F>(self, func: F) -> Sink<In, Mat2>
  where
    F: FnOnce(Mat) -> Mat2, {
    let (graph, mat) = self.into_parts();
    Sink::from_graph(graph, func(mat))
  }

  /// Transforms the input type of this sink by applying a mapping function.
  #[must_use]
  pub fn contramap<In2, F>(self, func: F) -> Sink<In2, Mat>
  where
    In2: Send + Sync + 'static,
    F: Fn(In2) -> In + Send + Sync + 'static, {
    let flow = super::flow::Flow::<In2, In, StreamNotUsed>::from_function(func);
    flow.to_mat(self, super::keep_right::KeepRight)
  }

  /// Enables restart semantics with backoff for this sink.
  #[must_use]
  pub fn restart_sink_with_backoff(mut self, min_backoff_ticks: u32, max_restarts: usize) -> Self {
    self.graph.set_sink_restart(Some(RestartBackoff::new(min_backoff_ticks, max_restarts)));
    self
  }

  /// Enables restart semantics by explicit restart settings.
  #[must_use]
  pub fn restart_sink_with_settings(mut self, settings: RestartSettings) -> Self {
    self.graph.set_sink_restart(Some(RestartBackoff::from_settings(settings)));
    self
  }

  /// Applies stop supervision semantics to this sink.
  #[must_use]
  pub fn supervision_stop(mut self) -> Self {
    self.graph.set_sink_supervision(SupervisionStrategy::Stop);
    self
  }

  /// Applies resume supervision semantics to this sink.
  #[must_use]
  pub fn supervision_resume(mut self) -> Self {
    self.graph.set_sink_supervision(SupervisionStrategy::Resume);
    self
  }

  /// Applies restart supervision semantics to this sink.
  #[must_use]
  pub fn supervision_restart(mut self) -> Self {
    self.graph.set_sink_supervision(SupervisionStrategy::Restart);
    self
  }

  fn from_definition<L>(kind: StageKind, logic: L, mat: Mat) -> Self
  where
    L: SinkLogic + 'static, {
    let inlet: Inlet<In> = Inlet::new();
    let definition = SinkDefinition {
      kind,
      inlet: inlet.id(),
      input_type: TypeId::of::<In>(),
      mat_combine: MatCombine::KeepRight,
      supervision: SupervisionStrategy::Stop,
      restart: None,
      logic: Box::new(logic),
    };
    let mut graph = StreamGraph::new();
    graph.push_stage(StageDefinition::Sink(definition));
    Self { graph, mat, _pd: PhantomData }
  }
}

impl<In, Mat> StreamStage for Sink<In, Mat> {
  type In = In;
  type Out = StreamNotUsed;

  fn shape(&self) -> StreamShape<Self::In, Self::Out> {
    let inlet = self.graph.head_inlet().map(Inlet::from_id).unwrap_or_default();
    StreamShape::new(inlet, Outlet::new())
  }
}

struct LazySinkLogic<In, F> {
  factory:    Option<F>,
  inner:      Option<Box<dyn SinkLogic>>,
  completion: StreamCompletion<StreamDone>,
  _pd:        PhantomData<fn(In)>,
}

impl<In, F> SinkLogic for LazySinkLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnOnce() -> Sink<In, StreamCompletion<StreamDone>> + Send + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    if let Some(factory) = self.factory.take() {
      let sink = factory();
      let (graph, _inner_completion) = sink.into_parts();
      let stages = graph.into_stages();
      for stage in stages {
        if let StageDefinition::Sink(def) = stage {
          self.inner = Some(def.logic);
          break;
        }
      }
      // ライフサイクル契約: inner sink のbackpressure初期化
      if let Some(inner) = &mut self.inner {
        inner.on_start(demand)?;
      }
    }

    match &mut self.inner {
      | Some(inner) => inner.on_push(input, demand),
      | None => {
        self.completion.complete(Err(StreamError::Failed));
        Ok(SinkDecision::Complete)
      },
    }
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    let result = match &mut self.inner {
      | Some(inner) => inner.on_complete(),
      | None => Ok(()),
    };
    match &result {
      | Ok(()) => self.completion.complete(Ok(StreamDone::new())),
      | Err(e) => self.completion.complete(Err(e.clone())),
    }
    result
  }

  fn on_error(&mut self, error: StreamError) {
    if let Some(inner) = &mut self.inner {
      inner.on_error(error.clone());
    }
    self.completion.complete(Err(error));
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    if let Some(inner) = &mut self.inner {
      inner.on_restart()?;
    }
    Ok(())
  }
}

struct IgnoreSinkLogic<In> {
  completion: StreamCompletion<StreamDone>,
  _pd:        PhantomData<fn(In)>,
}

impl<In> SinkLogic for IgnoreSinkLogic<In>
where
  In: Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, _input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
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

impl<In> GraphStageLogic<In, StreamNotUsed, StreamCompletion<StreamDone>> for IgnoreSinkLogic<In>
where
  In: Send + Sync + 'static,
{
  fn on_start(&mut self, ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    ctx.pull();
  }

  fn on_push(&mut self, ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    let _ = ctx.grab();
    ctx.pull();
  }

  fn on_complete(&mut self, _ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    self.completion.complete(Ok(StreamDone::new()));
  }

  fn on_error(&mut self, _ctx: &mut dyn StageContext<In, StreamNotUsed>, error: StreamError) {
    self.completion.complete(Err(error));
  }

  fn materialized(&mut self) -> StreamCompletion<StreamDone> {
    self.completion.clone()
  }
}

impl<In> GraphStage<In, StreamNotUsed, StreamCompletion<StreamDone>> for IgnoreSinkLogic<In>
where
  In: Send + Sync + 'static,
{
  fn shape(&self) -> StreamShape<In, StreamNotUsed> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<In, StreamNotUsed, StreamCompletion<StreamDone>>> {
    Box::new(IgnoreSinkLogic { completion: self.completion.clone(), _pd: PhantomData })
  }
}

struct ForeachSinkLogic<In, F> {
  func:       F,
  completion: StreamCompletion<StreamDone>,
  _pd:        PhantomData<fn(In)>,
}

impl<In, F> SinkLogic for ForeachSinkLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(In) + Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = downcast_value::<In>(input)?;
    (self.func)(value);
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

impl<In, F> GraphStageLogic<In, StreamNotUsed, StreamCompletion<StreamDone>> for ForeachSinkLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(In) + Send + Sync + 'static,
{
  fn on_start(&mut self, ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    ctx.pull();
  }

  fn on_push(&mut self, ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    let value = ctx.grab();
    (self.func)(value);
    ctx.pull();
  }

  fn on_complete(&mut self, _ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    self.completion.complete(Ok(StreamDone::new()));
  }

  fn on_error(&mut self, _ctx: &mut dyn StageContext<In, StreamNotUsed>, error: StreamError) {
    self.completion.complete(Err(error));
  }

  fn materialized(&mut self) -> StreamCompletion<StreamDone> {
    self.completion.clone()
  }
}

impl<In, F> GraphStage<In, StreamNotUsed, StreamCompletion<StreamDone>> for ForeachSinkLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(In) + Send + Sync + Clone + 'static,
{
  fn shape(&self) -> StreamShape<In, StreamNotUsed> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<In, StreamNotUsed, StreamCompletion<StreamDone>>> {
    Box::new(ForeachSinkLogic {
      func:       self.func.clone(),
      completion: self.completion.clone(),
      _pd:        PhantomData,
    })
  }
}

struct FoldSinkLogic<In, Acc, F> {
  acc:        Option<Acc>,
  func:       F,
  completion: StreamCompletion<Acc>,
  _pd:        PhantomData<fn(In)>,
}

impl<In, Acc, F> SinkLogic for FoldSinkLogic<In, Acc, F>
where
  In: Send + Sync + 'static,
  Acc: Send + Sync + 'static,
  F: FnMut(Acc, In) -> Acc + Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = downcast_value::<In>(input)?;
    let Some(current) = self.acc.take() else {
      return Err(StreamError::Failed);
    };
    let next = (self.func)(current, value);
    self.acc = Some(next);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    let Some(value) = self.acc.take() else {
      return Err(StreamError::Failed);
    };
    self.completion.complete(Ok(value));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

impl<In, Acc, F> GraphStageLogic<In, StreamNotUsed, StreamCompletion<Acc>> for FoldSinkLogic<In, Acc, F>
where
  In: Send + Sync + 'static,
  Acc: Send + Sync + 'static,
  F: FnMut(Acc, In) -> Acc + Send + Sync + 'static,
{
  fn on_start(&mut self, ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    ctx.pull();
  }

  fn on_push(&mut self, ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    let value = ctx.grab();
    let Some(current) = self.acc.take() else {
      ctx.fail(StreamError::Failed);
      return;
    };
    let next = (self.func)(current, value);
    self.acc = Some(next);
    ctx.pull();
  }

  fn on_complete(&mut self, _ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    if let Some(value) = self.acc.take() {
      self.completion.complete(Ok(value));
    } else {
      self.completion.complete(Err(StreamError::Failed));
    }
  }

  fn on_error(&mut self, _ctx: &mut dyn StageContext<In, StreamNotUsed>, error: StreamError) {
    self.completion.complete(Err(error));
  }

  fn materialized(&mut self) -> StreamCompletion<Acc> {
    self.completion.clone()
  }
}

impl<In, Acc, F> GraphStage<In, StreamNotUsed, StreamCompletion<Acc>> for FoldSinkLogic<In, Acc, F>
where
  In: Send + Sync + 'static,
  Acc: Send + Sync + Clone + 'static,
  F: FnMut(Acc, In) -> Acc + Send + Sync + Clone + 'static,
{
  fn shape(&self) -> StreamShape<In, StreamNotUsed> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<In, StreamNotUsed, StreamCompletion<Acc>>> {
    Box::new(FoldSinkLogic {
      acc:        self.acc.clone(),
      func:       self.func.clone(),
      completion: self.completion.clone(),
      _pd:        PhantomData,
    })
  }
}

struct HeadSinkLogic<In> {
  completion: StreamCompletion<In>,
  seen:       bool,
  _pd:        PhantomData<fn(In)>,
}

impl<In> SinkLogic for HeadSinkLogic<In>
where
  In: Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, _demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    if self.seen {
      return Ok(SinkDecision::Complete);
    }
    let value = downcast_value::<In>(input)?;
    self.seen = true;
    self.completion.complete(Ok(value));
    Ok(SinkDecision::Complete)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    if !self.seen {
      self.completion.complete(Err(StreamError::Failed));
    }
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

impl<In> GraphStageLogic<In, StreamNotUsed, StreamCompletion<In>> for HeadSinkLogic<In>
where
  In: Send + Sync + 'static,
{
  fn on_start(&mut self, ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    ctx.pull();
  }

  fn on_push(&mut self, ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    if self.seen {
      ctx.complete();
      return;
    }
    let value = ctx.grab();
    self.seen = true;
    self.completion.complete(Ok(value));
    ctx.complete();
  }

  fn on_complete(&mut self, _ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    if !self.seen {
      self.completion.complete(Err(StreamError::Failed));
    }
  }

  fn on_error(&mut self, _ctx: &mut dyn StageContext<In, StreamNotUsed>, error: StreamError) {
    self.completion.complete(Err(error));
  }

  fn materialized(&mut self) -> StreamCompletion<In> {
    self.completion.clone()
  }
}

impl<In> GraphStage<In, StreamNotUsed, StreamCompletion<In>> for HeadSinkLogic<In>
where
  In: Send + Sync + 'static + Clone,
{
  fn shape(&self) -> StreamShape<In, StreamNotUsed> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<In, StreamNotUsed, StreamCompletion<In>>> {
    Box::new(HeadSinkLogic { completion: self.completion.clone(), seen: false, _pd: PhantomData })
  }
}

struct LastSinkLogic<In> {
  last:       Option<In>,
  completion: StreamCompletion<In>,
  _pd:        PhantomData<fn(In)>,
}

impl<In> SinkLogic for LastSinkLogic<In>
where
  In: Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.last = Some(value);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    match self.last.take() {
      | Some(value) => self.completion.complete(Ok(value)),
      | None => self.completion.complete(Err(StreamError::Failed)),
    }
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

impl<In> GraphStageLogic<In, StreamNotUsed, StreamCompletion<In>> for LastSinkLogic<In>
where
  In: Send + Sync + 'static,
{
  fn on_start(&mut self, ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    ctx.pull();
  }

  fn on_push(&mut self, ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    let value = ctx.grab();
    self.last = Some(value);
    ctx.pull();
  }

  fn on_complete(&mut self, _ctx: &mut dyn StageContext<In, StreamNotUsed>) {
    match self.last.take() {
      | Some(value) => self.completion.complete(Ok(value)),
      | None => self.completion.complete(Err(StreamError::Failed)),
    }
  }

  fn on_error(&mut self, _ctx: &mut dyn StageContext<In, StreamNotUsed>, error: StreamError) {
    self.completion.complete(Err(error));
  }

  fn materialized(&mut self) -> StreamCompletion<In> {
    self.completion.clone()
  }
}

impl<In> GraphStage<In, StreamNotUsed, StreamCompletion<In>> for LastSinkLogic<In>
where
  In: Send + Sync + 'static + Clone,
{
  fn shape(&self) -> StreamShape<In, StreamNotUsed> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<In, StreamNotUsed, StreamCompletion<In>>> {
    Box::new(LastSinkLogic { last: None, completion: self.completion.clone(), _pd: PhantomData })
  }
}

struct CancelledSinkLogic {
  completion: StreamCompletion<StreamDone>,
}

impl SinkLogic for CancelledSinkLogic {
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, _input: DynValue, _demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    self.completion.complete(Ok(StreamDone::new()));
    Ok(SinkDecision::Complete)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.completion.complete(Ok(StreamDone::new()));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

struct OnCompleteSinkLogic<In, F> {
  callback:   F,
  completion: StreamCompletion<StreamDone>,
  _pd:        PhantomData<fn(In)>,
}

impl<In, F> SinkLogic for OnCompleteSinkLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(Result<StreamDone, StreamError>) + Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, _input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    (self.callback)(Ok(StreamDone::new()));
    self.completion.complete(Ok(StreamDone::new()));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    (self.callback)(Err(error.clone()));
    self.completion.complete(Err(error));
  }
}

struct HeadOptionSinkLogic<In> {
  completion: StreamCompletion<Option<In>>,
  seen:       bool,
  _pd:        PhantomData<fn(In)>,
}

impl<In> SinkLogic for HeadOptionSinkLogic<In>
where
  In: Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, _demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    if self.seen {
      return Ok(SinkDecision::Complete);
    }
    let value = downcast_value::<In>(input)?;
    self.seen = true;
    self.completion.complete(Ok(Some(value)));
    Ok(SinkDecision::Complete)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    if !self.seen {
      self.completion.complete(Ok(None));
    }
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

struct LastOptionSinkLogic<In> {
  last:       Option<In>,
  completion: StreamCompletion<Option<In>>,
  _pd:        PhantomData<fn(In)>,
}

impl<In> SinkLogic for LastOptionSinkLogic<In>
where
  In: Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.last = Some(value);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.completion.complete(Ok(self.last.take()));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

struct ReduceSinkLogic<In, F> {
  acc:        Option<In>,
  func:       F,
  completion: StreamCompletion<In>,
  _pd:        PhantomData<fn(In)>,
}

impl<In, F> SinkLogic for ReduceSinkLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(In, In) -> In + Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = downcast_value::<In>(input)?;
    let next = match self.acc.take() {
      | Some(current) => (self.func)(current, value),
      | None => value,
    };
    self.acc = Some(next);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    match self.acc.take() {
      | Some(value) => self.completion.complete(Ok(value)),
      | None => self.completion.complete(Err(StreamError::Failed)),
    }
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

struct TakeLastSinkLogic<In> {
  limit:      usize,
  values:     VecDeque<In>,
  completion: StreamCompletion<Vec<In>>,
  _pd:        PhantomData<fn(In)>,
}

impl<In> SinkLogic for TakeLastSinkLogic<In>
where
  In: Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = downcast_value::<In>(input)?;
    if self.limit > 0 {
      self.values.push_back(value);
      while self.values.len() > self.limit {
        let _ = self.values.pop_front();
      }
    }
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    let values: Vec<In> = core::mem::take(&mut self.values).into_iter().collect();
    self.completion.complete(Ok(values));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}
