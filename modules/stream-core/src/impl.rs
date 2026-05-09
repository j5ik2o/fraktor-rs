//! Internal implementation packages mirroring Pekko's `impl` boundary.

mod default_operator_catalog;
mod default_operator_catalog_failure;
mod default_operator_catalog_fan_in;
mod default_operator_catalog_fan_out;
mod default_operator_catalog_hub;
mod default_operator_catalog_kill_switch;
mod default_operator_catalog_source;
mod default_operator_catalog_substream;
mod default_operator_catalog_timing;
mod default_operator_catalog_transform;
mod flow_fragment;
mod framing_error_kind;
pub(crate) mod fusing;
mod graph_chain_macro;
mod graph_dsl_builder;
mod graph_stage_flow_adapter;
mod graph_stage_flow_context;
pub(crate) mod hub;
pub(crate) mod interpreter;
#[cfg(feature = "compression")]
mod io;
pub(crate) mod materialization;
mod operator_catalog;
mod operator_contract;
mod operator_coverage;
mod operator_key;
mod port_ops;
pub mod queue;
mod restart_backoff;
mod reverse_port_ops;
mod stream_dsl_error;
mod stream_error;
mod stream_graph;
pub(crate) mod streamref;
mod timeout_kind;
mod validate_positive_argument;

pub use default_operator_catalog::DefaultOperatorCatalog;
pub use framing_error_kind::FramingErrorKind;
pub(crate) use graph_dsl_builder::GraphDslBuilder;
pub(crate) use graph_stage_flow_adapter::GraphStageFlowAdapter;
#[cfg(feature = "compression")]
pub use io::Compression;
pub use operator_catalog::OperatorCatalog;
pub use operator_contract::OperatorContract;
pub use operator_coverage::OperatorCoverage;
pub use operator_key::OperatorKey;
pub(crate) use restart_backoff::RestartBackoff;
pub use stream_dsl_error::StreamDslError;
pub use stream_error::StreamError;
pub(crate) use stream_graph::StreamGraph;
pub use timeout_kind::TimeoutKind;
pub(crate) use validate_positive_argument::validate_positive_argument;

#[cfg(test)]
mod tests;
