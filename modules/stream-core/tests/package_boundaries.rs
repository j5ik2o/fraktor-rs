use fraktor_stream_rs::core::{
  attributes::{
    AsyncBoundaryAttr, Attributes, CancellationStrategyKind, DispatcherAttribute, InputBuffer, LogLevel, LogLevels,
  },
  dsl::{Flow, Sink, Source},
  materialization::{KeepBoth, KeepLeft, KeepNone, KeepRight, RunnableGraph, StreamCompletion, StreamNotUsed},
};

#[test]
fn attributes_package_exports_attribute_contracts() {
  let attributes = Attributes::async_boundary()
    .and(Attributes::dispatcher("stream-dispatcher"))
    .and(Attributes::input_buffer(4, 16))
    .and(Attributes::log_levels(LogLevel::Info, LogLevel::Warning, LogLevel::Error))
    .and(Attributes::cancellation_strategy(CancellationStrategyKind::FailStage));

  assert!(attributes.contains::<AsyncBoundaryAttr>());
  assert_eq!(attributes.get::<DispatcherAttribute>().map(DispatcherAttribute::name), Some("stream-dispatcher"),);
  assert_eq!(attributes.get::<InputBuffer>().map(|buffer| (buffer.initial, buffer.max)), Some((4, 16)));
  assert_eq!(
    attributes.get::<LogLevels>().map(|levels| (levels.on_element, levels.on_finish, levels.on_failure)),
    Some((LogLevel::Info, LogLevel::Warning, LogLevel::Error)),
  );
  assert_eq!(attributes.get::<CancellationStrategyKind>().copied(), Some(CancellationStrategyKind::FailStage),);
}

#[test]
fn materialization_package_exports_public_composition_rules() {
  let _ = KeepBoth;
  let _ = KeepNone;

  let _graph: RunnableGraph<StreamCompletion<u32>> = Source::single(1_u32)
    .via_mat(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 1), KeepLeft)
    .into_mat(Sink::<u32, StreamCompletion<u32>>::head(), KeepRight);
}

#[test]
fn dsl_package_exports_primary_stream_surface() {
  let graph: RunnableGraph<StreamCompletion<u32>> = Source::single(2_u32)
    .via_mat(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value * 2), KeepLeft)
    .into_mat(Sink::<u32, StreamCompletion<u32>>::head(), KeepRight);

  let _ = graph;
}
