#[cfg(feature = "std")]
use core::any::TypeId;

use fraktor_stream_rs::core::{
  attributes::{
    AsyncBoundaryAttr, Attributes, CancellationStrategyKind, DispatcherAttribute, InputBuffer, LogLevel, LogLevels,
  },
  dsl::{Flow, Sink, Source},
  materialization::{KeepBoth, KeepLeft, KeepNone, KeepRight, RunnableGraph, StreamCompletion, StreamNotUsed},
};
#[cfg(feature = "std")]
use fraktor_stream_rs::std::{
  io::{FileIO, StreamConverters},
  materializer::{SystemMaterializer, SystemMaterializerId},
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

#[cfg(feature = "std")]
#[test]
fn std_packages_export_io_and_materializer_adapters() {
  assert_eq!(TypeId::of::<FileIO>(), TypeId::of::<FileIO>());
  assert_eq!(TypeId::of::<StreamConverters>(), TypeId::of::<StreamConverters>());
  assert_eq!(TypeId::of::<SystemMaterializer>(), TypeId::of::<SystemMaterializer>());
  assert_eq!(TypeId::of::<SystemMaterializerId>(), TypeId::of::<SystemMaterializerId>());
}
