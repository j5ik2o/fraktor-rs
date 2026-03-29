//! Crate-private contract aliases backing the public DSL surface.
//!
//! Public `core::dsl::*` types should depend on this module instead of
//! referencing `core::stage::*` directly, so the stage package stays internal.

pub(crate) type ActorSink = crate::core::stage::ActorSink;
pub(crate) type ActorSource = crate::core::stage::ActorSource;
pub(crate) type BidiFlow<InTop, OutTop, InBottom, OutBottom, Mat> =
  crate::core::stage::BidiFlow<InTop, OutTop, InBottom, OutBottom, Mat>;
pub(crate) type Flow<In, Out, Mat> = crate::core::stage::Flow<In, Out, Mat>;
pub(crate) type FlowGroupBySubFlow<In, Key, Out, Mat> = crate::core::stage::FlowGroupBySubFlow<In, Key, Out, Mat>;
pub(crate) type FlowMonitorImpl<Out> = crate::core::stage::FlowMonitorImpl<Out>;
pub(crate) type FlowSubFlow<In, Out, Mat> = crate::core::stage::FlowSubFlow<In, Out, Mat>;
pub(crate) type FlowWithContext<Ctx, In, Out, Mat> = crate::core::stage::FlowWithContext<Ctx, In, Out, Mat>;
pub(crate) type RestartFlow = crate::core::stage::RestartFlow;
pub(crate) type RestartSink = crate::core::stage::RestartSink;
pub(crate) type RestartSource = crate::core::stage::RestartSource;
pub(crate) type Sink<In, Mat> = crate::core::stage::Sink<In, Mat>;
pub(crate) type Source<Out, Mat> = crate::core::stage::Source<Out, Mat>;
pub(crate) type SourceGroupBySubFlow<Key, Out, Mat> = crate::core::stage::SourceGroupBySubFlow<Key, Out, Mat>;
pub(crate) type SourceSubFlow<Out, Mat> = crate::core::stage::SourceSubFlow<Out, Mat>;
pub(crate) type SourceWithContext<Ctx, Out, Mat> = crate::core::stage::SourceWithContext<Ctx, Out, Mat>;
pub(crate) type TailSource<Out> = crate::core::stage::TailSource<Out>;
pub(crate) type TopicPubSub = crate::core::stage::TopicPubSub;
